import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Shapes
import QtQuick.Window

ApplicationWindow {
    id: win
    width: 720; height: 504
    minimumWidth: 480; minimumHeight: 336
    visible: true
    title: deck.loaded ? deck.filename + " — nap" : "nap — Nice Audio Player"
    color: bg
    // Every surface is mixed from the three Omarchy colours, so the deck follows any theme, light or dark.
    readonly property color bg: deck.palette.background
    readonly property color fg: deck.palette.foreground
    readonly property color accent: deck.palette.accent
    readonly property bool dark: lum(bg) < 0.4
    readonly property color shell: Qt.tint(bg, Qt.alpha(fg, 0.07))
    readonly property color well: dark ? Qt.darker(bg, 1.7) : Qt.darker(bg, 1.2)
    readonly property color dim: Qt.tint(bg, Qt.alpha(fg, 0.5))
    readonly property color paper: Qt.tint(fg, Qt.alpha(bg, 0.1))
    readonly property color ink: bg
    readonly property color inkDim: Qt.tint(paper, Qt.alpha(bg, 0.62))
    // Some themes pick an accent too close to the background to carry a signal; lift those toward the foreground.
    readonly property color glow: contrast(accent, bg) >= 3 ? accent : Qt.tint(accent, Qt.alpha(fg, 0.6))
    readonly property color stripe: contrast(accent, paper) >= 1.12 ? accent : inkDim
    readonly property color accentInk: contrast(accent, fg) >= contrast(accent, bg) ? fg : bg
    readonly property string mono: "monospace"
    readonly property int winding: forwardKey.down ? 1 : rewindKey.down ? -1 : 0
    property bool helpVisible: false
    property string message: ""
    function lum(c) {
        function linear(v) { return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    }
    function contrast(a, b) {
        const x = lum(a), y = lum(b)
        return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
    }
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
    DropArea { id: dropZone; anchors.fill: parent; onDropped: drop => { if (drop.hasUrls) deck.openUrl(drop.urls[0]) } }

    Item {
        id: design
        width: 720; height: 504
        anchors.centerIn: parent
        scale: Math.min(win.width / width, win.height / height)

        Rectangle {
            id: cassette
            x: 40; y: 18; width: 640; height: 376; radius: 16
            color: shell; border.color: dropZone.containsDrag ? glow : Qt.alpha(fg, 0.26); border.width: dropZone.containsDrag ? 2 : 1
            Rectangle { x: 4; y: 4; width: parent.width - 8; height: parent.height - 8; radius: 12; color: "transparent"; border.color: Qt.alpha(fg, 0.06) }
            Repeater {
                model: [[15, 15, 30], [625, 15, -20], [15, 361, -55], [625, 361, 40], [320, 353, 10]]
                Rectangle {
                    required property var modelData
                    x: modelData[0] - 5.5; y: modelData[1] - 5.5; z: 1; width: 11; height: 11; radius: 5.5
                    color: well; border.color: Qt.alpha(fg, 0.24)
                    Rectangle { anchors.centerIn: parent; width: 7; height: 1; rotation: parent.modelData[2]; color: Qt.alpha(fg, 0.4) }
                }
            }

            Rectangle {
                id: label
                x: 30; y: 22; width: 580; height: 252; radius: 8
                color: paper
                Rectangle {
                    x: 20; y: 16; width: 30; height: 30; radius: 3; color: "transparent"; border.color: ink; border.width: 2
                    Text { anchors.centerIn: parent; text: "A"; font.family: mono; font.pixelSize: 18; font.weight: Font.Bold; color: ink }
                }
                Text { x: 62; y: 17; width: 498; text: deck.filename; elide: Text.ElideMiddle; font.family: mono; font.pixelSize: 19; font.weight: Font.Bold; color: ink }
                Rectangle { x: 20; y: 52; width: 540; height: 1; color: inkDim; opacity: 0.55 }
                Text { x: 20; y: 59; width: 390; text: deck.loaded ? deck.detail : "Drop an audio file here, or press O to open one"; elide: Text.ElideRight; font.family: mono; font.pixelSize: 10; color: inkDim }
                Text { x: 410; y: 58; width: 150; horizontalAlignment: Text.AlignRight; text: clock(deck.position) + " / " + clock(deck.duration); font.family: mono; font.pixelSize: 11; font.weight: Font.DemiBold; color: ink }
                // The label's last ruled line is the timeline: it inks over as the tape plays.
                Slider {
                    id: timeline; objectName: "timeline"; x: 20; y: 77; width: 540; height: 20
                    padding: 0
                    from: 0; to: Math.max(1, deck.duration); value: deck.position; enabled: deck.duration > 0
                    focusPolicy: Qt.NoFocus
                    Accessible.name: "Playback position"
                    onMoved: deck.seek(value)
                    background: Item {
                        Rectangle { y: 9; width: parent.width; height: 1; color: inkDim; opacity: 0.55 }
                        Repeater {
                            model: 21
                            Rectangle { required property int index; x: Math.round(index * (timeline.width - 1) / 20); y: 11; width: 1; height: index % 5 ? 3 : 6; color: inkDim; opacity: 0.55 }
                        }
                        Rectangle { y: 8; width: timeline.visualPosition * parent.width; height: 3; radius: 1; color: ink }
                        Rectangle {
                            visible: deck.bookmark >= 0 && deck.duration > 0
                            x: deck.bookmark / Math.max(1, deck.duration) * parent.width - 3.5; y: -1; width: 7; height: 7; rotation: 45; color: ink
                        }
                    }
                    handle: Rectangle {
                        x: timeline.visualPosition * (timeline.width - width); y: timeline.pressed || timeline.hovered ? 1 : 3
                        width: 3; height: timeline.pressed || timeline.hovered ? 17 : 13; radius: 1; color: ink
                        visible: timeline.enabled
                    }
                }
                Repeater {
                    model: [[160, 10], [174, 5], [183, 2]]
                    Rectangle { required property var modelData; y: modelData[0]; width: parent.width; height: modelData[1]; color: stripe }
                }

                Rectangle {
                    id: tapeWindow
                    x: 56; y: 108; width: 468; height: 130; radius: 12
                    color: well; border.color: Qt.tint(paper, Qt.alpha(bg, 0.75)); border.width: 2
                    Reel {
                        x: 6; y: 6; spinning: deck.playing; wind: win.winding; tape: 1 - deck.position / Math.max(1, deck.duration)
                        hub: paper; hole: well; pack: Qt.tint(well, Qt.alpha(fg, 0.3)); groove: Qt.tint(well, Qt.alpha(fg, 0.42))
                    }
                    Reel {
                        x: 344; y: 6; spinning: deck.playing; wind: win.winding; tape: deck.position / Math.max(1, deck.duration)
                        hub: paper; hole: well; pack: Qt.tint(well, Qt.alpha(fg, 0.3)); groove: Qt.tint(well, Qt.alpha(fg, 0.42))
                    }
                    Rectangle {
                        id: scope
                        x: 130; y: 23; width: 208; height: 84; radius: 4
                        color: Qt.tint(well, Qt.alpha(fg, 0.04)); border.color: Qt.alpha(fg, 0.14)
                        // The live waveform, drawn in character cells: one column per pair of samples.
                        property var trace: []
                        function refresh() {
                            const samples = deck.wave, next = []
                            for (let i = 0; i < 48; i++) {
                                const v = samples.length > 2 * i + 1 ? (samples[2 * i] + samples[2 * i + 1]) / 2 : 0
                                // A gentle curve keeps quiet passages moving without clipping loud ones.
                                const shaped = Math.sign(v) * Math.pow(Math.min(1, Math.abs(v) * 1.4), 0.7)
                                next.push(samples.length ? (trace[i] || 0) * 0.3 + shaped * 0.7 : 0)
                            }
                            trace = next
                        }
                        Connections { target: deck; function onWaveChanged() { scope.refresh() } }
                        Item {
                            x: 9; y: 9; width: 191; height: 67
                            Repeater {
                                model: 48
                                Rectangle {
                                    required property int index
                                    readonly property int cells: Math.round((scope.trace[index] || 0) * 8)
                                    x: index * 4; y: (8 - Math.max(0, cells)) * 4
                                    width: 3; height: (Math.abs(cells) + 1) * 4 - 1
                                    color: deck.playing ? glow : Qt.alpha(fg, 0.22)
                                }
                            }
                            Repeater {
                                model: 16
                                Rectangle { required property int index; y: index * 4 + 3; width: parent.width; height: 1; color: scope.color }
                            }
                        }
                    }
                }
            }

            // The ridge under the tape: its grip grooves are the volume control.
            Shape {
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: Qt.alpha(fg, 0.2); strokeWidth: 1; fillColor: Qt.alpha(fg, 0.045); joinStyle: ShapePath.RoundJoin
                    PathSvg { path: "M122 290 L518 290 L544 375.5 L96 375.5 Z" }
                }
            }
            Row {
                x: 155; y: 303; spacing: 3
                Repeater {
                    model: 55
                    Rectangle {
                        required property int index
                        readonly property bool lit: !deck.muted && index / 55 < deck.volume
                        width: 3; height: 20; radius: 1; color: lit ? glow : well; opacity: lit ? 0.85 : 1
                    }
                }
            }
            MouseArea {
                x: 149; y: 296; width: 342; height: 34; cursorShape: Qt.PointingHandCursor
                function adjust(x) { deck.setVolume((x - 6) / 327) }
                onPressed: mouse => adjust(mouse.x)
                onPositionChanged: mouse => { if (pressed) adjust(mouse.x) }
                onWheel: wheel => deck.setVolume(deck.volume + (wheel.angleDelta.y > 0 ? 0.05 : -0.05))
                Accessible.role: Accessible.Slider; Accessible.name: "Volume"
            }
            Repeater {
                model: [[190, 18, 9], [256, 10, 2], [384, 10, 2], [450, 18, 9]]
                Rectangle {
                    required property var modelData
                    x: modelData[0] - width / 2; y: 353 - height / 2; width: modelData[1]; height: width; radius: modelData[2]
                    color: well; border.color: Qt.alpha(fg, 0.16)
                }
            }
            Text { x: 32; y: 347; text: deck.muted ? "muted" : "vol " + Math.round(deck.volume * 100); font.family: mono; font.pixelSize: 10; color: dim }
            Text { x: 508; y: 347; width: 100; horizontalAlignment: Text.AlignRight; text: deck.duration > 0 ? "C-" + Math.max(1, Math.round(deck.duration / 60000)) : "blank"; font.family: mono; font.pixelSize: 10; color: dim }
        }

        Row {
            x: 40; y: 410; spacing: 6
            Transport { id: rewindKey; glyph: "rew"; caption: "REW"; seekDirection: -1; ink: fg; face: shell; well: win.well; enabled: deck.loaded; onWind: seconds => deck.skip(seconds) }
            Transport { objectName: "playKey"; glyph: deck.playing ? "pause" : "play"; caption: deck.playing ? "PAUSE" : "PLAY"; ink: accentInk; face: accent; well: win.well; engaged: deck.playing; onClicked: deck.loaded ? deck.toggle() : win.open() }
            Transport { id: forwardKey; objectName: "forwardKey"; glyph: "fwd"; caption: "FWD"; seekDirection: 1; ink: fg; face: shell; well: win.well; enabled: deck.loaded; onWind: seconds => deck.skip(seconds) }
            Transport { glyph: "stop"; caption: "STOP"; ink: fg; face: shell; well: win.well; enabled: deck.loaded; onClicked: deck.stop() }
            Transport { glyph: "open"; caption: "OPEN"; ink: fg; face: shell; well: win.well; onClicked: win.open() }
        }
        Row {
            x: 524; y: 410; spacing: 6
            Transport { implicitWidth: 48; glyph: "loop"; caption: "LOOP"; ink: fg; face: shell; well: win.well; lamp: glow; hasLamp: true; lit: deck.looping; engaged: deck.looping; onClicked: deck.toggleLoop() }
            Transport { implicitWidth: 48; glyph: "mark"; caption: "MARK"; ink: fg; face: shell; well: win.well; lamp: glow; hasLamp: true; lit: deck.bookmark >= 0; enabled: deck.loaded; onClicked: deck.saveBookmark() }
            Transport { implicitWidth: 48; glyph: "help"; caption: "KEYS"; ink: fg; face: shell; well: win.well; engaged: win.helpVisible; onClicked: win.helpVisible = true }
        }
    }
    Rectangle {
        visible: win.message.length > 0 && !win.helpVisible
        anchors.horizontalCenter: parent.horizontalCenter; y: 8 * design.scale
        width: Math.min(parent.width - 24, toast.implicitWidth + 28); height: toast.implicitHeight + 16
        radius: 5; color: bg; border.color: glow; z: 10
        Text { id: toast; anchors.centerIn: parent; width: Math.min(implicitWidth, win.width - 52); wrapMode: Text.Wrap; text: win.message; color: fg; font.family: mono; font.pixelSize: 12 }
    }
    Rectangle {
        anchors.fill: parent; visible: win.helpVisible; color: Qt.alpha(bg, 0.97); z: 20
        MouseArea { anchors.fill: parent; onClicked: win.helpVisible = false }
        Column {
            anchors.centerIn: parent; width: Math.min(440, parent.width - 48); spacing: 11
            scale: Math.min(1, (parent.height - 32) / implicitHeight)
            Text { text: "Keys"; font.family: mono; font.pixelSize: 18; font.weight: Font.Bold; color: fg; bottomPadding: 6 }
            Repeater {
                model: [["Space", "Play or pause"], ["← / →", "Back or forward five seconds"], ["↑ / ↓", "Volume"], ["M", "Mute"], ["S", "Stop"], ["L", "Loop"], ["B", "Bookmark this spot"], ["Shift+B", "Remove the bookmark"], ["Enter", "Go to the bookmark"], ["O", "Open a file"], ["?", "Show or hide these keys"], ["Q", "Quit"]]
                Row {
                    required property var modelData
                    width: parent.width
                    Text { width: 130; text: modelData[0]; font.family: mono; font.pixelSize: 12; color: glow }
                    Text { text: modelData[1]; font.family: mono; font.pixelSize: 12; color: fg }
                }
            }
            Text { topPadding: 8; width: parent.width; wrapMode: Text.Wrap; text: "Drag the ruled line on the label to seek. Tap REW or FWD to jump five seconds, or hold to wind. Drag or scroll the grooves under the tape to set the volume."; color: dim; font.family: mono; font.pixelSize: 11; lineHeight: 1.45 }
            Text { text: "Esc or a click closes this."; color: dim; font.family: mono; font.pixelSize: 11 }
        }
    }
}
