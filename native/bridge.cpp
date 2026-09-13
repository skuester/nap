#include "player.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickWindow>
#include <QTimer>

static void initializeResources() {
    Q_INIT_RESOURCE(resources);
}

extern "C" int nap_run(const char *json) {
    initializeResources();
    const auto options = QJsonDocument::fromJson(json).object();
    int argc = 1; char name[] = "nap"; char *argv[] = {name, nullptr};
    QGuiApplication app(argc, argv);
    app.setApplicationName("nap"); app.setOrganizationName("nap"); app.setDesktopFileName("nap");
    Player player;
    player.setVolume(options["volume"].toDouble(0.75));
    if (options["looping"].toBool()) player.toggleLoop();
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("deck", &player);
    engine.load(QUrl("qrc:/src/Main.qml"));
    if (engine.rootObjects().isEmpty()) return 1;
    const auto path = options["path"].toString();
    if (!path.isEmpty()) player.openFile(path, options["paused"].toBool(), options["start"].toInteger(-1), options["ignore"].toBool());
    const auto screenshot = options["screenshot"].toString();
    if (!screenshot.isEmpty()) {
        QTimer::singleShot(1200, &app, [&] {
            auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
            app.exit(window && window->grabWindow().save(screenshot) ? 0 : 1);
        });
    }
    return app.exec();
}
