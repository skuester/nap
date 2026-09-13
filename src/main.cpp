#include "player.h"
#include "util.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QCommandLineParser>
#include <QQuickWindow>
#include <QTimer>
#include <QFileInfo>
#include <cstdio>

int main(int argc, char **argv) {
    // Handle CLI diagnostics without requiring a running display server.
    bool cliOnly = false;
    for (int i = 1; i < argc; ++i) {
        const QByteArray arg(argv[i]);
        if (arg == "--help" || arg == "-h" || arg == "--version" || arg == "-V") cliOnly = true;
    }
    if (cliOnly) {
        qputenv("QT_QPA_PLATFORM", "offscreen");
        qputenv("QT_QPA_PLATFORMTHEME", "generic");
    }
    QGuiApplication app(argc, argv);
    app.setApplicationName("nap");
    app.setApplicationVersion("0.1.0");
    app.setOrganizationName("nap");
    app.setDesktopFileName("nap");
    QCommandLineParser parser;
    parser.setApplicationDescription("Nice Audio Player — a little cassette deck for your desktop.\n\nKeys: Space play/pause · S stop · arrows seek/volume · B bookmark\nShift+B remove bookmark · Enter return to bookmark · L loop · M mute\nO open · ? help · Q quit");
    parser.addHelpOption(); parser.addVersionOption();
    parser.addPositionalArgument("audio", "Local audio file (or open one in the player).", "[audio]");
    parser.addOption({"paused", "Load without starting playback."});
    parser.addOption({{"time", "start", "timestamp"}, "Start at seconds, m:ss, or h:mm:ss; overrides bookmark.", "time"});
    parser.addOption({"ignore-bookmark", "Start at the beginning."});
    parser.addOption({"volume", "Initial volume, 0–100.", "percent", "75"});
    parser.addOption({"loop", "Repeat the tape."});
    parser.addOption({"screenshot", "Save a UI preview as PNG and exit (for development).", "path"});
    parser.process(app);
    auto fail = [](const QString &s) { fprintf(stderr, "nap: %s\n", qPrintable(s)); return 2; };
    if (parser.positionalArguments().size() > 1) return fail("open one audio file at a time");
    qint64 start = -1;
    if (parser.isSet("time")) {
        auto parsed = timestamp(parser.value("time"));
        if (!parsed) return fail("invalid timestamp");
        start = *parsed;
    }
    bool ok;
    double volume = parser.value("volume").toDouble(&ok);
    if (!ok || !std::isfinite(volume) || volume < 0 || volume > 100) return fail("volume must be between 0 and 100");
    const QString file = parser.positionalArguments().value(0);
    if (!file.isEmpty() && (!QFileInfo(file).isFile() || !QFileInfo(file).isReadable())) return fail("file is not readable: " + file);
    Player player;
    player.setVolume(volume / 100);
    if (parser.isSet("loop")) player.toggleLoop();
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("deck", &player);
    engine.load(QUrl("qrc:/src/Main.qml"));
    if (engine.rootObjects().isEmpty()) return 1;
    if (!file.isEmpty()) player.openFile(file, parser.isSet("paused"), start, parser.isSet("ignore-bookmark"));
    if (parser.isSet("screenshot")) {
        QTimer::singleShot(1200, &app, [&] {
            auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
            app.exit(window && window->grabWindow().save(parser.value("screenshot")) ? 0 : 1);
        });
    }
    return app.exec();
}
