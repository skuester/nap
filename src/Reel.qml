import QtQuick

// One spool: a tape pack that grows and shrinks, around a toothed hub that turns only while spinning.
Item {
    id: reel
    property bool spinning: false
    property real tape: 0.8
    property int wind: 0
    property color hub: "#dcd8c9"
    property color hole: "#0d1012"
    property color pack: "#342e29"
    property color groove: "#51463a"
    readonly property real packSize: 58 + Math.max(0, Math.min(1, tape)) * 58
    width: 118; height: 118
    Rectangle {
        anchors.centerIn: parent
        width: reel.packSize; height: width; radius: width / 2
        color: reel.pack; border.color: reel.groove
        Repeater {
            model: 4
            Rectangle {
                required property int index
                anchors.centerIn: parent
                visible: width > 56
                width: parent.width - 10 - index * 12; height: width; radius: width / 2
                color: "transparent"; border.color: reel.groove; opacity: 0.45
            }
        }
    }
    Item {
        id: spindle
        anchors.centerIn: parent; width: 52; height: 52
        Rectangle { anchors.fill: parent; radius: 26; color: reel.hub }
        Rectangle { anchors.centerIn: parent; width: 34; height: 34; radius: 17; color: reel.hole }
        Repeater {
            model: 6
            Item {
                required property int index
                anchors.fill: parent; rotation: index * 60
                Rectangle { anchors.horizontalCenter: parent.horizontalCenter; y: 8; width: 6; height: 8; radius: 1; color: reel.hub }
            }
        }
    }
    // Tape speed is constant, so a thin pack turns faster than a full one. Side A turns anticlockwise.
    FrameAnimation {
        running: reel.spinning
        onTriggered: spindle.rotation = (spindle.rotation - frameTime * 150 * 58 / reel.packSize * (reel.wind ? reel.wind * 9 : 1)) % 360
    }
}
