import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Window

ApplicationWindow {
    id: win
    width: 720; height: 504
    minimumWidth: 480; minimumHeight: 336
    visible: true
    title: deck.loaded ? deck.filename + " — nap" : "nap — Nice Audio Player"
    color: bg
    readonly property color bg: deck.palette.background
    readonly property color fg: deck.palette.foreground
    readonly property color accent: deck.palette.accent
    readonly property color panel: Qt.tint(bg, Qt.alpha(fg, 0.045))
    readonly property color dim: Qt.tint(bg, Qt.alpha(fg, 0.48))
    property bool helpVisible: false
    property string message: ""
    function clock(ms) {
        let s = Math.max(0, Math.floor(ms / 1000))
        return (s >= 3600 ? Math.floor(s / 3600) + ":" : "") + (s >= 3600 ? String(Math.floor(s / 60) % 60).padStart(2, "0") : String(Math.floor(s / 60)).padStart(2, "0")) + ":" + String(s % 60).padStart(2, "0")
    }
    function open() { picker.open() }
    Shortcut { sequence: "Space"; onActivated: deck.toggle() }
    Shortcut { sequence: "S"; onActivated: deck.stop() }
    Shortcut { sequence: "Left"; onActivated: deck.skip(-5) }
    Shortcut { sequence: "Right"; onActivated: deck.skip(5) }
    Shortcut { sequence: "Up"; onActivated: deck.setVolume(deck.volume + 0.05) }
    Shortcut { sequence: "Down"; onActivated: deck.setVolume(deck.volume - 0.05) }
    Shortcut { sequence: "B"; onActivated: deck.saveBookmark() }
    Shortcut { sequence: "Shift+B"; onActivated: deck.saveBookmark(true) }
    Shortcut { sequence: "Return"; onActivated: if (deck.bookmark >= 0) deck.seek(deck.bookmark) }
    Shortcut { sequence: "L"; onActivated: deck.toggleLoop() }
    Shortcut { sequence: "M"; onActivated: deck.toggleMute() }
    Shortcut { sequence: "O"; onActivated: win.open() }
    Shortcut { sequence: "Ctrl+O"; onActivated: win.open() }
    Shortcut { sequence: "Q"; onActivated: Qt.quit() }
    Shortcut { sequence: "?"; onActivated: win.helpVisible = !win.helpVisible }
    Shortcut { sequence: "K"; onActivated: win.helpVisible = !win.helpVisible }
    Shortcut { sequence: "Escape"; onActivated: win.helpVisible = false }
    FileDialog {
        id: picker; title: "Load a tape"
        nameFilters: ["Audio (*.mp3 *.flac *.wav *.ogg *.opus *.m4a *.aac *.aiff *.aif *.wma *.ape *.alac *.wv)", "All files (*)"]
        onAccepted: deck.openUrl(selectedFile)
    }
    Connections {
        target: deck
        function onNotice(text) { win.message = text; toastTimer.restart() }
    }
    Timer { id: toastTimer; interval: 6500; onTriggered: win.message = "" }
    DropArea { anchors.fill: parent; onDropped: drop => { if (drop.hasUrls) deck.openUrl(drop.urls[0]) } }

    Item {
        id: design
        width: 720; height: 504
        readonly property real deckSpacing: 24
        anchors.centerIn: parent
        scale: Math.min(win.width / width, win.height / height)

        Rectangle {
            id: tapeHousing
            x: 30; y: design.deckSpacing; width: 660; height: 350; radius: 14
            color: "#0c1012"; border.color: Qt.tint(bg, Qt.alpha(fg, 0.15))
            Rectangle {
                x: 5; y: 4; width: 650; height: 340; radius: 11
                color: panel; border.color: Qt.tint(bg, Qt.alpha(fg, 0.12))
            }
            Rectangle {
                id: cassette
                x: 25; y: 21; width: 610; height: 306; radius: 15
                color: Qt.tint(bg, Qt.alpha(fg, 0.085)); border.color: Qt.tint(bg, Qt.alpha(fg, 0.25)); border.width: 1
                Rectangle { x: 3; y: 3; width: parent.width - 6; height: parent.height - 7; radius: 13; color: "transparent"; border.color: Qt.alpha(fg, 0.045) }

                Rectangle {
                    x: 28; y: 21; width: 554; height: 100; radius: 4
                    color: "#d9d5c5"
                    Rectangle { x: 0; y: 0; width: 7; height: parent.height; color: accent; radius: 2 }
                    Text { x: 21; y: 12; text: "A"; font.pixelSize: 30; font.weight: Font.Bold; color: "#303532" }
                    Text { x: 58; y: 14; text: "PERSONAL MAGNETIC AUDIO"; font.family: "monospace"; font.pixelSize: 8; font.letterSpacing: 1.6; color: "#686b60" }
                    Text { x: 58; y: 33; width: 473; text: deck.filename; elide: Text.ElideMiddle; font.pixelSize: 20; font.weight: Font.Medium; color: "#292f2d" }
                    Text { x: 21; y: 65; width: 370; text: deck.loaded ? deck.detail : "Open a file or drop one onto the deck"; elide: Text.ElideRight; font.family: "monospace"; font.pixelSize: 9; color: "#686b60" }
                    Text { x: 410; y: 65; width: 122; horizontalAlignment: Text.AlignRight; text: clock(deck.position) + " / " + clock(deck.duration); font.family: "monospace"; font.pixelSize: 10; color: "#303532" }
                    Slider {
                        id: timeline; objectName: "timeline"; x: 20; y: 82; width: 513; height: 18
                        from: 0; to: Math.max(1, deck.duration); value: deck.position; enabled: deck.duration > 0
                        focusPolicy: Qt.NoFocus
                        Accessible.name: "Playback position"
                        onMoved: deck.seek(value)
                        background: Rectangle {
                            x: timeline.leftPadding; y: timeline.topPadding + timeline.availableHeight / 2 - 1
                            width: timeline.availableWidth; height: 2; color: "#aaa998"
                            Rectangle { width: timeline.visualPosition * parent.width; height: 2; color: "#383f36" }
                            Rectangle { visible: deck.bookmark >= 0 && deck.duration > 0; x: deck.bookmark / Math.max(1, deck.duration) * parent.width - 2; y: -4; width: 4; height: 10; color: "#956638" }
                        }
                        handle: Rectangle { x: timeline.leftPadding + timeline.visualPosition * (timeline.availableWidth - width); y: 4; width: 4; height: 10; radius: 1; color: "#303832" }
                    }
                }

                Rectangle {
                    x: 50; y: 136; width: 510; height: 137; radius: 68
                    color: "#131719"; border.color: Qt.alpha(fg, 0.17)
                    Reel { x: 4; y: 3; spinning: deck.playing; tape: 1 - deck.position / Math.max(1, deck.duration) }
                    Reel { x: 376; y: 3; spinning: deck.playing; tape: deck.position / Math.max(1, deck.duration) }
                    Rectangle {
                        x: 150; y: 29; width: 210; height: 77; radius: 5
                        color: "#0d1315"; border.color: "#333d3b"
                        Repeater { model: 5; Rectangle { required property int index; x: 1; y: 13 + index * 13; width: 208; height: 1; color: "#23302f"; opacity: 0.4 } }
                        Canvas {
                            id: waveform; anchors.fill: parent; anchors.margins: 9
                            onPaint: {
                                const ctx = getContext("2d"); ctx.reset()
                                ctx.strokeStyle = win.accent; ctx.lineWidth = 1.5
                                ctx.beginPath()
                                const samples = deck.wave
                                for (let i = 0; i < 96; i++) {
                                    const v = samples.length > i ? samples[i] : 0
                                    const x = i * width / 95, y = height / 2 - v * height * 0.46
                                    if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y)
                                }
                                ctx.stroke()
                            }
                            Connections { target: deck; function onWaveChanged() { waveform.requestPaint() } }
                        }
                        Text { anchors.horizontalCenter: parent.horizontalCenter; y: 84; text: "NAP · NICE AUDIO PLAYER"; font.family: "monospace"; font.pixelSize: 8; font.letterSpacing: 0.8; color: "#737c73" }
                    }
                }
                Repeater {
                    model: [[13,13],[584,13],[13,280],[584,280]]
                    Rectangle {
                        required property var modelData
                        x: modelData[0]; y: modelData[1]; width: 12; height: 12; radius: 6
                        color: "#161c1d"; border.color: Qt.alpha(fg, 0.23)
                        Rectangle { anchors.centerIn: parent; width: 6; height: 1; rotation: 35; color: Qt.alpha(fg, 0.35) }
                    }
                }
                Rectangle {
                    x: 154; y: 282; width: 302; height: 24; radius: 3
                    color: Qt.tint(bg, Qt.alpha(fg, 0.07)); border.color: Qt.alpha(fg, 0.15)
                    Row {
                        x: 10; y: 5; spacing: 3
                        Repeater { model: 47; Rectangle { required property int index; width: 3; height: 14; radius: 1; color: !deck.muted && index / 47 < deck.volume ? win.accent : "#111719"; opacity: !deck.muted && index / 47 < deck.volume ? 0.65 : 1 } }
                    }
                    MouseArea {
                        anchors.fill: parent; cursorShape: Qt.PointingHandCursor
                        function adjust(x) { deck.setVolume((x - 10) / 282) }
                        onPressed: mouse => adjust(mouse.x)
                        onPositionChanged: mouse => { if (pressed) adjust(mouse.x) }
                        onWheel: wheel => deck.setVolume(deck.volume + (wheel.angleDelta.y > 0 ? 0.05 : -0.05))
                        Accessible.role: Accessible.Slider; Accessible.name: "Volume"
                    }
                }
                Text { x: 80; y: 287; text: deck.muted ? "MUTED" : "VOL " + Math.round(deck.volume * 100); font.family: "monospace"; font.pixelSize: 8; color: dim }
                Text { x: 473; y: 287; text: "TYPE I · NAP"; font.family: "monospace"; font.pixelSize: 8; color: dim }
            }
        }

        Row {
            id: transport
            x: 36; y: tapeHousing.y + tapeHousing.height + design.deckSpacing; spacing: 7
            Transport { symbol: "◀◀"; caption: "REW"; seekDirection: -1; ink: fg; face: panel; enabled: deck.loaded; onWind: seconds => deck.skip(seconds) }
            Transport { objectName: "playKey"; symbol: deck.playing ? "Ⅱ" : "▶"; caption: deck.playing ? "PAUSE" : "PLAY"; ink: bg; face: accent; engaged: deck.playing; onClicked: deck.loaded ? deck.toggle() : win.open() }
            Transport { objectName: "forwardKey"; symbol: "▶▶"; caption: "FWD"; seekDirection: 1; ink: fg; face: panel; enabled: deck.loaded; onWind: seconds => deck.skip(seconds) }
            Transport { symbol: "■"; caption: "STOP"; ink: fg; face: panel; enabled: deck.loaded; onClicked: deck.stop() }
            Transport { symbol: "⏏"; caption: "OPEN"; ink: fg; face: panel; onClicked: win.open() }
        }
        Column {
            x: 616; y: transport.y + 5; spacing: 15
            TextMetrics { id: actionCell; font.family: "monospace"; font.pixelSize: 10; text: "M" }
            Repeater {
                model: ["LOOP", "MARK", "KEYS"]
                Item {
                    required property int index
                    required property string modelData
                    width: actionCell.advanceWidth * 6; height: actionLabel.implicitHeight
                    readonly property color ink: (index === 0 && deck.looping) || (index === 1 && deck.bookmark >= 0) ? accent : dim
                    Item {
                        width: actionCell.advanceWidth; height: parent.height
                        Text {
                            anchors.centerIn: parent
                            visible: index !== 1; text: index === 0 ? "↻" : "?"
                            font: actionCell.font; color: actionLabel.color
                        }
                        Rectangle {
                            anchors.centerIn: parent; visible: index === 1
                            width: 4; height: 10; color: "#956638"
                        }
                    }
                    Text {
                        id: actionLabel
                        x: actionCell.advanceWidth * 2; text: modelData
                        font: actionCell.font; color: parent.ink
                    }
                    MouseArea {
                        anchors.fill: parent; anchors.margins: -6; cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (index === 0) deck.toggleLoop()
                            else if (index === 1) deck.saveBookmark()
                            else win.helpVisible = true
                        }
                    }
                }
            }
        }
    }
    Rectangle {
        visible: win.message.length > 0 && !win.helpVisible
        anchors.horizontalCenter: parent.horizontalCenter; y: 8 * design.scale
        width: Math.min(parent.width - 24, toast.implicitWidth + 28); height: toast.implicitHeight + 18
        radius: 5; color: fg; z: 10
        Text { id: toast; anchors.centerIn: parent; width: Math.min(implicitWidth, win.width - 52); wrapMode: Text.Wrap; text: win.message; color: bg; font.pixelSize: 12 }
    }
    Rectangle {
        anchors.fill: parent; visible: win.helpVisible; color: Qt.alpha(bg, 0.97); z: 20
        MouseArea { anchors.fill: parent; onClicked: win.helpVisible = false }
        Column {
            anchors.centerIn: parent; width: Math.min(420, parent.width - 48); spacing: 16
            scale: Math.min(1, (parent.height - 32) / implicitHeight)
            Text { text: "A little deck. A few good keys."; font.pixelSize: 22; color: fg }
            Repeater {
                model: [["Space", "Play / pause"], ["← / →", "Back / forward five seconds"], ["↑ / ↓ · M", "Volume / mute"], ["S · L", "Stop / toggle loop"], ["B · Shift+B", "Save / remove bookmark"], ["Enter", "Return to bookmark"], ["O · Ctrl+O", "Open a file"], ["? · K · Esc", "Help / close help"], ["Q", "Quit"]]
                Row {
                    required property var modelData
                    width: parent.width
                    Text { width: 150; text: modelData[0]; font.family: "monospace"; font.pixelSize: 12; color: accent }
                    Text { text: modelData[1]; font.pixelSize: 12; color: fg }
                }
            }
            Text { text: "Drag the label’s underline to seek.\nTap REW / FWD for 5s; hold to wind.\nThe cassette’s lower ridges control volume."; color: dim; font.pixelSize: 12; lineHeight: 1.5 }
            Text { text: "CLOSE  ×"; color: accent; font.family: "monospace"; font.pixelSize: 11 }
        }
    }
}
