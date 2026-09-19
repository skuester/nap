#include <QtTest>
#include "../native/player.h"
#include <QTemporaryDir>
#include <QFile>
#include <QDataStream>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickWindow>
#include <QQuickItem>
#include <QDesktopServices>
#include <cmath>

class PlayerTest : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    QString audio;
    QUrl opened;
public slots:
    void openedUrl(const QUrl &url) { opened = url; }
private slots:
    void initTestCase() {
        audio = directory.filePath("A test tape.wav");
        QFile file(audio); QVERIFY(file.open(QFile::WriteOnly));
        QDataStream s(&file); s.setByteOrder(QDataStream::LittleEndian);
        const quint32 bytes = 48000 * 2 * 12;
        s.writeRawData("RIFF", 4); s << quint32(bytes + 36); s.writeRawData("WAVEfmt ", 8);
        s << quint32(16) << quint16(1) << quint16(1) << quint32(48000) << quint32(96000) << quint16(2) << quint16(16);
        s.writeRawData("data", 4); s << bytes;
        for (int i = 0; i < 48000 * 12; ++i) s << qint16(std::sin(i * 440.0 * 6.283185 / 48000) * 8000);
    }
    void playbackAndBookmarks() {
        Player p; p.setVolume(0); QSignalSpy notices(&p, &Player::notice);
        p.openFile(audio, true, 2000);
        QTRY_VERIFY_WITH_TIMEOUT(p.duration() == 12000, 5000);
        QTRY_COMPARE(p.position(), 2000);
        QVERIFY(!p.playing());
        p.skip(5); QCOMPARE(p.position(), 7000);
        p.saveBookmark(); QCOMPARE(p.bookmark(), 7000);
        QCOMPARE(notices.last()[0].toString(), QString("Bookmarked"));
        const QString renamed = directory.filePath("Renamed tape.wav");
        QVERIFY(QFile::rename(audio, renamed)); audio = renamed;
        p.openFile(audio, true);
        QTRY_COMPARE(p.position(), 7000);
        QCOMPARE(p.bookmark(), 7000);
        p.openFile(audio, true, 1000); QTRY_COMPARE(p.position(), 1000);
        p.openFile(audio, true, -1, true); QTRY_COMPARE(p.position(), 0);
        p.seek(-1000); QCOMPARE(p.position(), 0);
        p.seek(999999); QCOMPARE(p.position(), p.duration());
        p.seek(1000); p.toggle();
        QTRY_VERIFY(p.playing());
        QTRY_VERIFY_WITH_TIMEOUT(p.position() > 1100, 5000);
        QTRY_VERIFY_WITH_TIMEOUT(!p.wave().isEmpty(), 5000);
        bool nonzero = false;
        for (const auto &v : p.wave()) nonzero |= std::abs(v.toDouble()) > 0.01;
        QVERIFY(nonzero);
        p.toggle(); QVERIFY(!p.playing()); QVERIFY(p.wave().isEmpty());
        p.toggleLoop(); QVERIFY(p.looping()); p.toggleLoop(); QVERIFY(!p.looping());
        p.stop(); QCOMPARE(p.position(), 0); QVERIFY(!p.playing());
        p.saveBookmark(true); QCOMPARE(p.bookmark(), -1);
        p.saveBookmark(true); QCOMPARE(p.bookmark(), -1);
        p.setVolume(-5); QCOMPARE(p.volume(), 0.0);
        p.setVolume(5); QCOMPARE(p.volume(), 1.0);
        p.toggleMute(); QVERIFY(p.muted());
        // Catch the file-manager launch instead of opening a real window.
        QDesktopServices::setUrlHandler("file", this, "openedUrl");
        p.openFolder(); QCOMPARE(opened, QUrl::fromLocalFile(QFileInfo(audio).absolutePath()));
        QDesktopServices::unsetUrlHandler("file");
        p.openFile(directory.filePath("missing.wav"));
        QVERIFY(notices.last()[0].toString().contains("Cannot open"));
    }
    void keyboardAndWindow() {
        Player p; p.setVolume(0);
        QQmlApplicationEngine engine;
        engine.rootContext()->setContextProperty("deck", &p);
        QSignalSpy warnings(&engine, &QQmlEngine::warnings);
        engine.load(QUrl("qrc:/src/Main.qml"));
        QVERIFY(!engine.rootObjects().isEmpty());
        auto *w = qobject_cast<QQuickWindow *>(engine.rootObjects().first()); QVERIFY(w);
        QVERIFY(QTest::qWaitForWindowExposed(w));
        p.openFile(audio, true); QTRY_COMPARE(p.duration(), 12000);
        QTest::keyClick(w, Qt::Key_Space); QTRY_VERIFY(p.playing());
        QTest::keyClick(w, Qt::Key_Space); QTRY_VERIFY(!p.playing());
        QTest::keyClick(w, Qt::Key_Right); QVERIFY(p.position() >= 5000);
        QTest::keyClick(w, Qt::Key_B); QVERIFY(p.bookmark() >= 5000);
        QTest::keyClick(w, Qt::Key_S); QCOMPARE(p.position(), 0);
        QTest::keyClick(w, Qt::Key_Return); QVERIFY(p.position() >= 5000);
        QTest::keyClick(w, Qt::Key_B, Qt::ShiftModifier); QCOMPARE(p.bookmark(), -1);
        QTest::keyClick(w, Qt::Key_L); QVERIFY(p.looping());
        QTest::keyClick(w, Qt::Key_I); QVERIFY(w->property("insertVisible").toBool());
        QCOMPARE(p.insert()["file"].toList().first().toList().last().toString(), QString("WAV"));
        QTest::qWait(700); QVERIFY(w->grabWindow().save("build/insert-preview.png"));
        QTest::keyClick(w, Qt::Key_Escape); QVERIFY(!w->property("insertVisible").toBool());
        auto *badge = w->findChild<QQuickItem *>("insertBadge"); QVERIFY(badge);
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, badge->mapToScene(QPointF(15, 15)).toPoint());
        QVERIFY(w->property("insertVisible").toBool());
        QTest::keyClick(w, Qt::Key_I); QVERIFY(!w->property("insertVisible").toBool());
        QTest::keyClick(w, Qt::Key_K); QVERIFY(w->property("helpVisible").toBool());
        QTest::keyClick(w, Qt::Key_Escape); QVERIFY(!w->property("helpVisible").toBool());
        auto *key = w->findChild<QQuickItem *>("playKey"); QVERIFY(key);
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, key->mapToScene(QPointF(50, 40)).toPoint());
        QTRY_VERIFY(p.playing()); p.stop();
        auto *forward = w->findChild<QQuickItem *>("forwardKey"); QVERIFY(forward);
        const auto point = forward->mapToScene(QPointF(50, 40)).toPoint();
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, point);
        QCOMPARE(p.position(), 5000);
        p.seek(0);
        QTest::mousePress(w, Qt::LeftButton, Qt::NoModifier, point);
        QTest::qWait(600);
        QTest::mouseRelease(w, Qt::LeftButton, Qt::NoModifier, point);
        QVERIFY(p.position() >= 9000);
        const auto released = p.position(); QTest::qWait(200); QCOMPARE(p.position(), released);
        auto *timeline = w->findChild<QQuickItem *>("timeline"); QVERIFY(timeline);
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, timeline->mapToScene(QPointF(timeline->width() / 2, 9)).toPoint());
        QVERIFY(std::abs(p.position() - 6000) < 200);
        p.toggle(); QTest::qWait(120);
        QVERIFY(w->grabWindow().save("build/playing-preview.png"));
        p.stop();
        w->resize(480, 336); QTest::qWait(100);
        QVERIFY(!w->grabWindow().isNull());
        QCOMPARE(w->minimumWidth() * 504, w->minimumHeight() * 720);
        QTest::keyClick(w, Qt::Key_K);
        QVERIFY(!w->grabWindow().isNull());
        QCOMPARE(warnings.count(), 0);
    }
};
QTEST_MAIN(PlayerTest)
#include "player_test.moc"
