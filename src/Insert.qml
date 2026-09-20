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
    // The tape in the deck. With more than one track, a name, or a cover of its own, the front
    // panel becomes a track listing; `card` then describes whichever track is picked out.
    property var tape: ({})
    property int selected: -1
    property bool typing: false
    property var writing: null
    signal dismissed()
    signal folderRequested()
    signal filesDropped(var urls)
    signal tapeDropped(var urls)
    signal coverDropped(url image)
    signal renamed(string name)
    signal signed(string from)
    signal noted(string note)
    // Something lifted off the card for a closer look: the cover, or the note tucked in with the
    // tape. Neither ever lands quite straight.
    property string held: ""
    readonly property bool zoomed: held !== ""
    property real tilt: 0
    function lift(what) { tilt = (1 + Math.random() * 1.6) * (Math.random() < 0.5 ? -1 : 1); held = what }
    function zoom() { lift("cover") }
    function putBack() { if (writing) writing.end(true); held = "" }
    signal playRequested(int index)
    signal moveRequested(int from, int to)
    signal removeRequested(int index)
    readonly property var tracks: tape.tracks || []
    readonly property bool mixtape: tape.mixtape === true
    readonly property int current: tape.index || 0
    readonly property int shown: selected >= 0 && selected < tracks.length ? selected : current
    readonly property bool tagged: (card.tags || []).length > 0
    readonly property string trackTitle: card.title || (tracks[shown] || {}).file || filename
    readonly property string title: tape.name || (mixtape ? "Untitled tape" : trackTitle)
    readonly property string byline: mixtape ? summary : card.artist || (tagged ? "" : "No tags in this file")
    readonly property string summary: tracks.length + (tracks.length === 1 ? " track, " : " tracks, ") + Math.max(1, Math.round((tape.seconds || 0) / 60)) + " min"
    readonly property string spineNote: mixtape ? summary : card.artist || ""
    function minutes(seconds) { return Math.floor(seconds / 60) + ":" + String(seconds % 60).padStart(2, "0") }
    onOpenChanged: if (!open) { selected = -1; typing = false; held = "" }
    readonly property string pressing: [card.album, card.year].filter(part => part).join(", ")
    // 0 is the card as it sits in the case; 1 is the notes panel folded all the way out.
    property real unfold: open ? 1 : 0
    function scroll(step) { notes.contentY = Math.max(0, Math.min(Math.max(0, notes.contentHeight - notes.height), notes.contentY + step)) }
    onCardChanged: notes.contentY = 0
    visible: opacity > 0
    enabled: open

    // Type set in place: a double click makes the words themselves editable, with no field around them.
    component Editable: Item {
        id: editable
        property string text
        property int size: 15
        property int lines: 1
        property real leading: 1.0
        property int weight: Font.Bold
        property bool italic: false
        // Shown faintly while there is nothing written yet.
        property string placeholder: ""
        // A note runs to several lines: Enter breaks the line, Ctrl+Enter or a click elsewhere keeps it.
        property bool multiline: false
        // What the editor opens with, where that differs from what is shown.
        property string draft: text
        readonly property bool editing: editor.visible
        signal committed(string text)
        implicitHeight: label.implicitHeight
        Text {
            id: label
            width: parent.width; visible: !editor.visible
            text: editable.text || editable.placeholder; wrapMode: Text.Wrap; maximumLineCount: editable.lines; elide: Text.ElideRight
            font.family: insert.mono; font.pixelSize: editable.size; font.weight: editable.weight; font.italic: editable.italic
            lineHeight: editable.leading; color: editable.text ? insert.ink : insert.inkDim; opacity: editable.text ? 1 : 0.75
            MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onDoubleClicked: editable.begin() }
        }
        // TextEdit has an insert() of its own, so everything that means the card lives out here.
        readonly property color ink: insert.ink
        readonly property color paper: insert.paper
        function begin() { editor.text = draft; editor.visible = true; insert.typing = true; insert.writing = editable; editor.forceActiveFocus(); editor.selectAll() }
        function end(keep) {
            if (!editor.visible) return
            editor.visible = false; insert.typing = false; insert.writing = null
            if (keep && editor.text.trim() !== draft) committed(editor.text.trim())
        }
        Connections { target: insert; function onOpenChanged() { editable.end(false) } }
        TextEdit {
            id: editor
            width: parent.width; visible: false
            wrapMode: TextEdit.Wrap; selectByMouse: true; textFormat: TextEdit.PlainText
            font: label.font; color: editable.ink; selectionColor: editable.ink; selectedTextColor: editable.paper
            Keys.onReturnPressed: event => { if (editable.multiline && !(event.modifiers & Qt.ControlModifier)) event.accepted = false; else editable.end(true) }
            Keys.onEnterPressed: editable.end(true)
            Keys.onEscapePressed: editable.end(false)
            onActiveFocusChanged: if (!activeFocus) editable.end(true)
        }
    }
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
        id: lifted
        anchors.fill: parent; z: 6
        visible: opacity > 0; opacity: insert.zoomed ? 1 : 0; enabled: insert.zoomed
        Behavior on opacity { NumberAnimation { duration: 140 } }
        Rectangle { anchors.fill: parent; anchors.margins: -2000; color: Qt.alpha(insert.scrim, 0.55) }
        // A click puts it back; while the note is being written, the first click only keeps the words.
        MouseArea { anchors.fill: parent; anchors.margins: -2000; onClicked: insert.writing ? insert.writing.end(true) : insert.putBack(); onWheel: wheel => wheel.accepted = true }
        Rectangle {
            id: photo
            readonly property real side: Math.min(parent.width, parent.height) - 96
            // Kept to the picture's own shape, within the square the card allows.
            readonly property real shape: closeUp.implicitWidth > 0 ? closeUp.implicitHeight / closeUp.implicitWidth : 1
            visible: insert.held !== "note"
            anchors.centerIn: parent; anchors.verticalCenterOffset: insert.zoomed ? 0 : 14
            width: (shape > 1 ? side / shape : side) + 20; height: (shape > 1 ? side : side * shape) + 20
            rotation: insert.tilt; scale: insert.zoomed ? 1 : 0.94; antialiasing: true
            color: insert.paper; radius: 2
            Behavior on scale { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
            Rectangle { z: -1; x: 6; y: 9; width: parent.width; height: parent.height; radius: 3; color: Qt.alpha("black", 0.35); antialiasing: true }
            Image { id: closeUp; anchors.fill: parent; anchors.margins: 10; source: insert.zoomed || lifted.visible ? picture.source : ""; fillMode: Image.PreserveAspectFit; asynchronous: true; mipmap: true; smooth: true }
        }
        // The note that came with the tape: a slip of ruled paper, written on directly.
        Rectangle {
            visible: slip.visible
            x: slip.x + 6; y: slip.y + 9; width: slip.width; height: slip.height; radius: 3
            rotation: slip.rotation; scale: slip.scale; color: Qt.alpha("black", 0.4); antialiasing: true
        }
        Rectangle {
            id: slip
            visible: insert.held === "note"
            anchors.centerIn: parent; anchors.verticalCenterOffset: insert.zoomed ? 0 : 14
            width: 400; height: Math.max(220, Math.min(400, words.implicitHeight + 96))
            rotation: insert.tilt; scale: insert.zoomed ? 1 : 0.94; antialiasing: true
            color: Qt.tint(insert.paper, Qt.alpha("white", 0.12)); radius: 2; clip: true
            Behavior on scale { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
            FontMetrics { id: hand; font.family: insert.mono; font.pixelSize: 13; font.italic: true }
            // Clicks on the paper are for writing, not for putting it away.
            MouseArea { anchors.fill: parent; onClicked: if (insert.writing) insert.writing.end(true); onDoubleClicked: words.begin() }
            Repeater {
                model: Math.floor((slip.height - 56) / hand.lineSpacing)
                Rectangle { required property int index; x: 28; y: 34 + (index + 1) * hand.lineSpacing; width: slip.width - 56; height: 1; color: insert.inkDim; opacity: 0.35 }
            }
            Rectangle { x: 20; width: 1; height: parent.height; color: insert.stripe; opacity: 0.6 }
            Editable {
                id: words
                x: 28; y: 34 + hand.lineSpacing - hand.ascent - 3; width: slip.width - 56
                text: insert.tape.note || ""; placeholder: "Write a note to go with this tape"
                size: 13; lines: 17; weight: Font.Normal; italic: true; multiline: true
                onCommitted: text => insert.noted(text)
            }
            Text { visible: !!insert.tape.from; anchors.right: parent.right; anchors.rightMargin: 28; anchors.bottom: parent.bottom; anchors.bottomMargin: 18; text: insert.tape.from || ""; font: hand.font; color: insert.ink }
        }
        // An empty slip is for writing on, so it arrives ready.
        Connections { target: insert; function onHeldChanged() { if (insert.held === "note" && !insert.tape.note) words.begin() } }
    }

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
        // A press anywhere but the words being written keeps them, as clicking away from type should.
        MouseArea {
            anchors.fill: parent; anchors.margins: -2000; z: 4; enabled: insert.writing !== null
            onPressed: mouse => {
                const at = mapToItem(insert.writing, mouse.x, mouse.y)
                if (at.x < 0 || at.y < 0 || at.x > insert.writing.width || at.y > insert.writing.height) insert.writing.end(true)
                mouse.accepted = false
            }
        }
        DropArea {
            id: drops
            anchors.fill: parent; z: 3
            property bool overArt: false
            function image(urls) { return urls.length > 0 && /\.(png|jpe?g|webp|gif|bmp)$/i.test(urls[0].toString()) }
            onPositionChanged: drag => overArt = drag.hasUrls && image(drag.urls) && drag.x < sheet.coverWidth
            onExited: overArt = false
            onDropped: drop => {
                overArt = false
                if (!drop.hasUrls) return
                if (image(drop.urls)) insert.coverDropped(drop.urls[0])
                else if (/\.(tape|jcard)$/i.test(drop.urls[0].toString())) insert.tapeDropped(drop.urls)
                else insert.filesDropped(drop.urls)
            }
        }

        Rectangle {
            id: front
            width: sheet.coverWidth; height: parent.height; radius: 2; color: insert.paper
            Rectangle {
                id: art
                readonly property int side: insert.mixtape ? 112 : 260
                x: 16; y: 16; width: side; height: side; clip: true
                color: Qt.tint(insert.paper, Qt.alpha(insert.ink, 0.1))
                // Without art, the cover is set in type, the way a home-dubbed tape's would be.
                Item {
                    anchors.fill: parent; visible: picture.status !== Image.Ready
                    Repeater {
                        model: [[96, 22], [126, 11], [145, 4]]
                        Rectangle { required property var modelData; y: modelData[0] * art.side / 260; width: parent.width; height: Math.max(1, modelData[1] * art.side / 260); color: insert.stripe }
                    }
                    Text { visible: !insert.mixtape; x: 16; y: 14; width: 228; text: insert.byline; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 11; color: insert.inkDim }
                    Editable {
                        visible: !insert.mixtape
                        x: 16; width: 228; anchors.bottom: parent.bottom; anchors.bottomMargin: 14
                        text: insert.title; size: 24; lines: 3; leading: 1.05
                        onCommitted: text => insert.renamed(text)
                    }
                }
                Image {
                    id: picture
                    anchors.fill: parent; source: insert.tape.coverUrl && insert.tape.coverUrl.toString() ? insert.tape.coverUrl : insert.card.cover || ""
                    fillMode: Image.PreserveAspectCrop; asynchronous: true; mipmap: true
                    sourceSize.width: 780; sourceSize.height: 780
                    MouseArea { anchors.fill: parent; enabled: picture.status === Image.Ready; cursorShape: Qt.PointingHandCursor; onClicked: insert.zoom(); Accessible.role: Accessible.Button; Accessible.name: "Look at the cover" }
                }
                Rectangle { anchors.fill: parent; color: "transparent"; border.color: drops.overArt ? insert.ink : Qt.alpha(insert.ink, 0.35); border.width: drops.overArt ? 3 : 1 }
            }

            // One file: the cover with its title beneath.
            Item {
                visible: !insert.mixtape
                x: 16; y: 292; width: 260
                Editable { width: parent.width; text: insert.title; onCommitted: text => insert.renamed(text) }
                Text { y: 24; width: parent.width; text: insert.byline; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 11; color: insert.ink }
                Text { y: 42; width: parent.width; text: insert.pressing; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
            }

            // A mixtape: its name beside a small cover, and the track listing.
            Item {
                visible: insert.mixtape
                x: 140; y: 16; width: 136
                Editable { id: tapeName; width: parent.width; text: insert.title; size: 14; lines: 3; leading: 1.1; onCommitted: text => insert.renamed(text) }
                Text { id: tapeSummary; y: tapeName.implicitHeight + 8; width: parent.width; text: insert.byline; wrapMode: Text.Wrap; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
                Editable {
                    id: signature
                    y: tapeSummary.y + tapeSummary.implicitHeight + 10; width: parent.width
                    text: insert.tape.from ? "from " + insert.tape.from : ""; placeholder: "Sign your name"
                    size: 11; lines: 2; weight: Font.Normal; italic: true
                    // The word "from" is printed on the card; only the name is yours to write.
                    draft: insert.tape.from || ""
                    onCommitted: text => insert.signed(text)
                }
                Text {
                    y: signature.y + signature.implicitHeight + 6; width: parent.width
                    text: insert.tape.note ? "Read the note" : "Tuck in a note"
                    font.family: insert.mono; font.pixelSize: 11; font.italic: true; font.underline: noteLink.containsMouse
                    color: insert.tape.note ? insert.ink : insert.inkDim; opacity: insert.tape.note ? 1 : 0.75
                    MouseArea { id: noteLink; anchors.fill: parent; anchors.margins: -3; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: insert.lift("note"); Accessible.role: Accessible.Button; Accessible.name: parent.text }
                }
            }
            Rectangle { visible: insert.mixtape; x: 16; y: 140; width: 260; height: 1; color: insert.inkDim; opacity: 0.55 }
            ListView {
                id: listing
                visible: insert.mixtape
                x: 10; y: 146; width: 272; height: 258; clip: true; interactive: false
                model: insert.tracks
                property int dragFrom: -1
                property int dropAt: -1
                delegate: Item {
                    id: row
                    required property int index
                    required property var modelData
                    readonly property bool up: index === insert.current
                    width: listing.width; height: 21.5
                    Rectangle { anchors.fill: parent; radius: 2; color: Qt.alpha(insert.ink, index === insert.shown ? 0.13 : hit.containsMouse ? 0.05 : 0) }
                    Text { x: 6; anchors.verticalCenter: parent.verticalCenter; text: row.up ? "▶" : String(row.index + 1).padStart(2, "0"); font.family: insert.mono; font.pixelSize: row.up ? 8 : 10; color: row.up ? insert.ink : insert.inkDim }
                    Text {
                        x: 28; width: parent.width - 74; anchors.verticalCenter: parent.verticalCenter
                        text: row.modelData.title || row.modelData.file; elide: Text.ElideRight
                        font.family: insert.mono; font.pixelSize: 10; font.weight: row.up ? Font.Bold : Font.Normal; color: insert.ink
                    }
                    Text { visible: !remove.visible; anchors.right: parent.right; anchors.rightMargin: 6; anchors.verticalCenter: parent.verticalCenter; text: insert.minutes(row.modelData.seconds || 0); font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
                    MouseArea {
                        id: hit
                        anchors.fill: parent; hoverEnabled: true; preventStealing: true
                        property real pressedAt: 0
                        onPressed: mouse => pressedAt = mouse.y
                        // Past a small threshold a press becomes a drag that reorders the listing.
                        onPositionChanged: mouse => {
                            if (!pressed || (listing.dragFrom < 0 && Math.abs(mouse.y - pressedAt) < 6)) return
                            listing.dragFrom = row.index
                            const y = mapToItem(listing.contentItem, 0, mouse.y).y
                            listing.dropAt = Math.max(0, Math.min(insert.tracks.length, Math.round(y / row.height)))
                        }
                        onReleased: {
                            const from = listing.dragFrom, gap = listing.dropAt
                            listing.dragFrom = -1; listing.dropAt = -1
                            if (from < 0) { insert.selected = row.index; return }
                            const to = gap > from ? gap - 1 : gap
                            if (to !== from) { insert.selected = to; insert.moveRequested(from, to) }
                        }
                        onCanceled: { listing.dragFrom = -1; listing.dropAt = -1 }
                        onDoubleClicked: insert.playRequested(row.index)
                        onWheel: wheel => listing.contentY = Math.max(0, Math.min(Math.max(0, listing.contentHeight - listing.height), listing.contentY - wheel.angleDelta.y / 2))
                    }
                    Text {
                        id: remove
                        visible: hit.containsMouse && insert.tracks.length > 1 && listing.dragFrom < 0
                        anchors.right: parent.right; anchors.rightMargin: 4; anchors.verticalCenter: parent.verticalCenter
                        width: 16; horizontalAlignment: Text.AlignHCenter; text: "×"; font.family: insert.mono; font.pixelSize: 13; color: removeArea.containsMouse ? insert.ink : insert.inkDim
                        MouseArea { id: removeArea; anchors.fill: parent; anchors.margins: -3; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: insert.removeRequested(row.index); Accessible.role: Accessible.Button; Accessible.name: "Remove from the tape" }
                    }
                }
                Rectangle { visible: listing.dropAt >= 0; x: 4; y: listing.dropAt * 21.5 - listing.contentY - 1; width: parent.width - 8; height: 2; color: insert.ink }
            }

            Rectangle { x: 16; y: 410; width: 260; height: 1; color: insert.inkDim; opacity: 0.55 }
            Text {
                x: 16; y: 420; width: 200; elide: Text.ElideRight; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim
                text: drops.containsDrag ? (drops.overArt ? "Drop to set the cover" : "Drop to add to the tape") : insert.tape.dirty ? "Unsaved. Ctrl+S saves the tape" : insert.mixtape ? "Drop audio here to add it" : (insert.card.file || []).slice(0, 1).map(row => row[1]).join("")
            }
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
                    text: "<b>" + insert.title.replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</b>" + (insert.spineNote ? "&nbsp;&nbsp;&nbsp;" + insert.spineNote.replace(/&/g, "&amp;").replace(/</g, "&lt;") : "")
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
                    Column {
                        visible: insert.mixtape
                        width: prose.width; spacing: 3; bottomPadding: 8
                        Text { width: parent.width; text: (insert.shown + 1) + ". " + insert.trackTitle; wrapMode: Text.Wrap; font.family: insert.mono; font.pixelSize: 13; font.weight: Font.Bold; color: insert.ink }
                        Text { visible: text.length > 0; width: parent.width; text: [insert.card.artist, insert.card.album, insert.card.year].filter(part => part).join(", "); wrapMode: Text.Wrap; font.family: insert.mono; font.pixelSize: 10; color: insert.inkDim }
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
