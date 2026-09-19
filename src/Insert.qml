import QtQuick

// The J-card: front cover, inked spine, and a liner-notes panel that unfolds from the spine.
Item {
    id: insert
    property bool open: false
    property var card: ({})
    property string filename: ""
    property string lengthClass: ""
    property color paper: "#d9d5c5"
    property color ink: "#1c1213"
    property color inkDim: "#686b60"
    property color stripe: "#684c59"
    property color scrim: "#000000"
    property string mono: "monospace"
    signal dismissed()
    signal folderRequested()
    readonly property bool tagged: (card.tags || []).length > 0
    readonly property string title: card.title || filename
    readonly property string byline: card.artist || (tagged ? "" : "No tags in this file")
    readonly property string pressing: [card.album, card.year].filter(part => part).join(", ")
    // 0 is the card as it sits in the case; 1 is the notes panel folded all the way out.
    property real unfold: open ? 1 : 0
    function scroll(step) { notes.contentY = Math.max(0, Math.min(Math.max(0, notes.contentHeight - notes.height), notes.contentY + step)) }
    onCardChanged: notes.contentY = 0
    visible: opacity > 0
    enabled: open
    opacity: open ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 160 } }
    Behavior on unfold {
        SequentialAnimation {
            PauseAnimation { duration: insert.open ? 150 : 0 }
            NumberAnimation { duration: insert.open ? 300 : 120; easing.type: Easing.OutCubic }
        }
    }
    Rectangle { anchors.fill: parent; anchors.margins: -2000; color: insert.scrim }
    MouseArea { anchors.fill: parent; anchors.margins: -2000; onClicked: insert.dismissed(); onWheel: wheel => wheel.accepted = true }

    Item {
        id: sheet
        readonly property int coverWidth: 292
        readonly property int spineWidth: 36
        readonly property int notesWidth: 336
        x: Math.round((parent.width - coverWidth - spineWidth - notesWidth * insert.unfold) / 2)
        y: 26 + (1 - insert.opacity) * 28
        width: coverWidth + spineWidth + notesWidth; height: 452
        MouseArea { width: sheet.coverWidth + sheet.spineWidth + sheet.notesWidth * insert.unfold; height: parent.height; onWheel: wheel => insert.scroll(-wheel.angleDelta.y / 2) }
        Rectangle { x: 5; y: 7; width: sheet.coverWidth + sheet.spineWidth + sheet.notesWidth * insert.unfold; height: parent.height; radius: 3; color: Qt.alpha("black", 0.3) }

        Rectangle {
            id: front
            width: sheet.coverWidth; height: parent.height; radius: 2; color: insert.paper
            Rectangle {
                id: art
                x: 16; y: 16; width: 260; height: 260; clip: true
                color: Qt.tint(insert.paper, Qt.alpha(insert.ink, 0.1))
                // Without embedded art, the cover is set in type, the way a home-dubbed tape's would be.
                Item {
                    anchors.fill: parent; visible: picture.status !== Image.Ready
                    Repeater {
                        model: [[96, 22], [126, 11], [145, 4]]
                        Rectangle { required property var modelData; y: modelData[0]; width: parent.width; height: modelData[1]; color: insert.stripe }
                    }
                    Text { x: 16; y: 14; width: 228; text: insert.byline; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 11; color: insert.inkDim }
                    Text {
                        x: 16; width: 228; anchors.bottom: parent.bottom; anchors.bottomMargin: 14
                        text: insert.title; wrapMode: Text.Wrap; maximumLineCount: 3; elide: Text.ElideRight
                        font.family: insert.mono; font.pixelSize: 24; font.weight: Font.Bold; lineHeight: 1.05; color: insert.ink
                    }
                }
                Image {
                    id: picture
                    anchors.fill: parent; source: insert.card.cover || ""
                    fillMode: Image.PreserveAspectCrop; asynchronous: true; mipmap: true
                    sourceSize.width: 780; sourceSize.height: 780
                }
                Rectangle { anchors.fill: parent; color: "transparent"; border.color: Qt.alpha(insert.ink, 0.35) }
            }
            Text { x: 16; y: 292; width: 260; text: insert.title; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 15; font.weight: Font.Bold; color: insert.ink }
            Text { x: 16; y: 316; width: 260; text: insert.byline; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 11; color: insert.ink }
            Text { x: 16; y: 334; width: 260; text: insert.pressing; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
            Rectangle { x: 16; y: 410; width: 260; height: 1; color: insert.inkDim; opacity: 0.55 }
            Text { x: 16; y: 420; text: (insert.card.file || []).slice(0, 1).map(row => row[1]).join(""); font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
            Text { x: 16; y: 420; width: 260; horizontalAlignment: Text.AlignRight; text: insert.lengthClass; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
            Rectangle {
                anchors.right: parent.right; width: 12; height: parent.height
                gradient: Gradient { orientation: Gradient.Horizontal; GradientStop { position: 0; color: "transparent" } GradientStop { position: 1; color: Qt.alpha(insert.ink, 0.16) } }
            }
        }

        Rectangle {
            id: spine
            x: sheet.coverWidth; width: sheet.spineWidth; height: parent.height; color: insert.ink
            Item {
                anchors.centerIn: parent; width: spine.height; height: spine.width; rotation: 90
                Text {
                    x: 16; width: parent.width - 96; anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.StyledText; elide: Text.ElideRight
                    text: "<b>" + insert.title.replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</b>" + (insert.card.artist ? "&nbsp;&nbsp;&nbsp;" + insert.card.artist.replace(/&/g, "&amp;").replace(/</g, "&lt;") : "")
                    font.family: insert.mono; font.pixelSize: 12; color: insert.paper
                }
                Text { anchors.right: parent.right; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter; text: insert.lengthClass; font.family: insert.mono; font.pixelSize: 11; color: insert.stripe === insert.inkDim ? insert.paper : Qt.tint(insert.stripe, Qt.alpha(insert.paper, 0.45)) }
            }
        }

        Rectangle {
            id: liner
            x: sheet.coverWidth + sheet.spineWidth; width: sheet.notesWidth; height: parent.height; radius: 2; color: insert.paper
            transform: Scale { xScale: insert.unfold }
            visible: insert.unfold > 0.001
            Flickable {
                id: notes
                x: 22; y: 18; width: parent.width - 40; height: parent.height - 36
                contentHeight: prose.implicitHeight; clip: true; boundsBehavior: Flickable.StopAtBounds
                Column {
                    id: prose
                    width: notes.width; spacing: 5
                    component Heading: Column {
                        property string text
                        width: prose.width; topPadding: 6; bottomPadding: 3; spacing: 4
                        Text { text: parent.text; font.family: insert.mono; font.pixelSize: 12; font.weight: Font.Bold; color: insert.ink }
                        Rectangle { width: parent.width; height: 1; color: insert.inkDim; opacity: 0.55 }
                    }
                    component Rows: Column {
                        property var rows: []
                        property string link: ""
                        signal followed()
                        width: prose.width; spacing: 5
                        Repeater {
                            model: parent.rows
                            Row {
                                id: entry
                                required property var modelData
                                readonly property bool linked: modelData[0] === parent.link
                                width: prose.width; spacing: 8
                                Text { width: 104; text: modelData[0]; wrapMode: Text.Wrap; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
                                Text {
                                    width: parent.width - 112; text: modelData[1]; wrapMode: Text.Wrap; textFormat: Text.PlainText
                                    font.family: insert.mono; font.pixelSize: 10; font.underline: entry.linked; color: insert.ink
                                    opacity: follow.containsMouse ? 0.6 : 1
                                    MouseArea {
                                        id: follow
                                        anchors.fill: parent; enabled: entry.linked; hoverEnabled: true
                                        cursorShape: entry.linked ? Qt.PointingHandCursor : Qt.ArrowCursor
                                        onClicked: entry.parent.followed()
                                        Accessible.role: Accessible.Link; Accessible.name: "Open " + entry.modelData[1]
                                    }
                                }
                            }
                        }
                    }
                    Heading { text: "Lyrics"; visible: lyrics.visible; topPadding: 0 }
                    Text { id: lyrics; visible: text.length > 0; width: prose.width; text: insert.card.lyrics || ""; textFormat: Text.PlainText; wrapMode: Text.Wrap; lineHeight: 1.45; bottomPadding: 8; font.family: insert.mono; font.pixelSize: 10; color: insert.ink }
                    Heading { text: "Tags"; topPadding: lyrics.visible ? 6 : 0 }
                    Text { visible: !insert.tagged; width: prose.width; text: "This file carries no tags, so the label shows its name."; wrapMode: Text.Wrap; lineHeight: 1.45; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
                    Rows { rows: insert.card.tags || []; bottomPadding: 8 }
                    Heading { text: "File" }
                    Rows { rows: insert.card.file || []; link: "Folder"; onFollowed: insert.folderRequested() }
                }
            }
            Rectangle {
                visible: notes.contentHeight > notes.height
                x: parent.width - 9; width: 2; radius: 1; color: Qt.alpha(insert.ink, 0.45)
                y: notes.y + notes.visibleArea.yPosition * notes.height; height: Math.max(14, notes.visibleArea.heightRatio * notes.height)
            }
            Rectangle {
                width: 12; height: parent.height
                gradient: Gradient { orientation: Gradient.Horizontal; GradientStop { position: 0; color: Qt.alpha(insert.ink, 0.16) } GradientStop { position: 1; color: "transparent" } }
            }
        }
    }
}
