import QtQuick

Item {
    id: reel
    property bool spinning: false
    property real tape: 0.8
    property color ink: "#dcd8c9"
    width: 130; height: 130
    Rectangle {
        anchors.centerIn: parent
        width: 126; height: width; radius: width / 2
        color: "#0d1012"; border.color: "#484339"; border.width: 1
    }
    Rectangle {
        anchors.centerIn: parent
        width: 66 + reel.tape * 54; height: width; radius: width / 2
        color: "#342e29"; border.color: "#51463a"; border.width: 2
        Behavior on width { NumberAnimation { duration: 500 } }
        Repeater {
            model: 7
            Rectangle {
                required property int index
                anchors.centerIn: parent
                width: parent.width - 6 - index * 4; height: width; radius: width / 2
                color: "transparent"; border.color: "#625442"; opacity: 0.24
            }
        }
    }
    Item {
        id: hub
        anchors.centerIn: parent; width: 64; height: 64
        NumberAnimation on rotation {
            from: 0; to: 360; duration: 2800; loops: Animation.Infinite
            running: true; paused: !reel.spinning
        }
        Rectangle {
            anchors.fill: parent; radius: 32
            color: reel.ink; border.color: "#8b897d"; border.width: 2
        }
        Repeater {
            model: 6
            Item {
                required property int index
                anchors.fill: parent; rotation: index * 60
                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter; y: 5
                    width: 10; height: 15; radius: 3; color: "#272b2b"
                }
            }
        }
        Rectangle {
            anchors.centerIn: parent; width: 22; height: 22; radius: 11
            color: "#15191a"; border.color: "#99998c"; border.width: 3
        }
    }
}
