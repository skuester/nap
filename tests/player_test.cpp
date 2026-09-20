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
        // The 440 Hz tone lands in the analyzer's tenth bar and swings both needles to about -5 VU.
        QCOMPARE(p.spectrum().size(), 24);
        int loudest = 0;
        for (int i = 0; i < 24; ++i) if (p.spectrum()[i].toDouble() > p.spectrum()[loudest].toDouble()) loudest = i;
        QCOMPARE(loudest, 9);
        QCOMPARE(p.levels().size(), 4);
        QVERIFY(std::abs(p.levels()[2].toDouble() - (30 - 7.25) / 36) < 0.02);
        QVERIFY(std::abs(p.levels()[0].toDouble() - 0.39) < 0.03);
        QCOMPARE(p.levels()[0], p.levels()[1]);
        p.toggle(); QVERIFY(!p.playing()); QVERIFY(p.wave().isEmpty()); QVERIFY(p.spectrum().isEmpty());
        // The visualizer choice cycles and survives a relaunch; keep the user's own state out of it.
        QTemporaryDir state; const auto userState = qgetenv("XDG_STATE_HOME");
        qputenv("XDG_STATE_HOME", state.path().toUtf8());
        { Player first; QCOMPARE(first.visualizer(), QString("bars")); first.cycleVisualizer(); QCOMPARE(first.visualizer(), QString("vu")); }
        { Player relaunched; QCOMPARE(relaunched.visualizer(), QString("vu")); for (int i = 0; i < 3; ++i) relaunched.cycleVisualizer(); QCOMPARE(relaunched.visualizer(), QString("scope")); }
        if (userState.isEmpty()) qunsetenv("XDG_STATE_HOME"); else qputenv("XDG_STATE_HOME", userState);
        p.toggleLoop(); QVERIFY(p.looping()); p.toggleLoop(); QVERIFY(!p.looping());
        // STOP lifts the head and leaves the tape where it is; PREV and NEXT then find the start.
        QVERIFY(!p.stopped()); p.stop(); QVERIFY(p.stopped()); QVERIFY(!p.playing()); QVERIFY(p.position() > 1100);
        p.next(); QCOMPARE(p.position(), 0); p.seek(8000); p.previous(); QCOMPARE(p.position(), 0); QVERIFY(p.stopped());
        p.toggle(); QTRY_VERIFY(p.playing()); QVERIFY(!p.stopped()); p.stop();
        p.saveBookmark(true); QCOMPARE(p.bookmark(), -1);
        p.saveBookmark(true); QCOMPARE(p.bookmark(), -1);
        p.setVolume(-5); QCOMPARE(p.volume(), 0.0);
        p.setVolume(5); QCOMPARE(p.volume(), 1.0);
        p.toggleMute(); QVERIFY(p.muted());
        // Catch the file-manager launch instead of opening a real window.
        QDesktopServices::setUrlHandler("file", this, "openedUrl");
        p.openFolder(); QCOMPARE(opened, QUrl::fromLocalFile(QFileInfo(audio).absolutePath()));
        QDesktopServices::unsetUrlHandler("file");
        p.openUrl(QUrl("https://example.com/tape.mp3"));
        QCOMPARE(notices.last()[0].toString(), QString("Choose a local audio file"));
        p.openUrl(QUrl::fromLocalFile(audio)); QTRY_COMPARE(p.duration(), 12000);
        p.openFile(directory.filePath("missing.wav"));
        QVERIFY(notices.last()[0].toString().contains("Cannot open"));
    }
    void mixtape() {
        Player p; p.setVolume(0); QSignalSpy notices(&p, &Player::notice);
        const QString second = directory.filePath("Second.wav");
        QVERIFY(QFile::copy(audio, second));
        const QString first = QFileInfo(audio).fileName();
        const auto index = [&p] { return p.tape()["index"].toInt(); };
        const auto files = [&p] { QStringList names; for (const auto &t : p.tape()["tracks"].toList()) names << t.toMap()["file"].toString(); return names; };
        p.load({audio, second}, true);
        QTRY_COMPARE(p.duration(), 12000);
        QCOMPARE(files(), QStringList({first, "Second.wav"}));
        QVERIFY(p.tape()["mixtape"].toBool()); QVERIFY(!p.tape()["dirty"].toBool()); QCOMPARE(index(), 0);

        // With the head lifted, NEXT and PREV walk the loop of tracks and leave it lifted.
        p.stop(); p.next();
        QTRY_COMPARE(p.filename(), QString("Second.wav")); QTRY_COMPARE(p.duration(), 12000);
        QVERIFY(p.stopped()); QCOMPARE(index(), 1);
        p.next(); QTRY_COMPARE(p.filename(), first); QTRY_COMPARE(p.duration(), 12000); QVERIFY(p.stopped());
        p.previous(); QTRY_COMPARE(p.filename(), QString("Second.wav")); QTRY_COMPARE(p.duration(), 12000);
        p.seek(8000); p.previous(); QCOMPARE(p.position(), 0); QCOMPARE(index(), 1);

        // A track running out plays the next; the tape running out stops, cued back at its top.
        p.playTrack(0); QTRY_VERIFY(p.playing()); QCOMPARE(index(), 0);
        p.seek(11850); QTRY_COMPARE_WITH_TIMEOUT(index(), 1, 5000); QTRY_VERIFY_WITH_TIMEOUT(p.playing(), 5000);
        p.seek(11850); QTRY_VERIFY_WITH_TIMEOUT(p.stopped(), 5000); QTRY_COMPARE(index(), 0);
        QTRY_COMPARE(p.filename(), first);
        // Looping, it starts over instead.
        p.toggleLoop(); p.playTrack(1); QTRY_VERIFY(p.playing());
        p.seek(11850); QTRY_COMPARE_WITH_TIMEOUT(index(), 0, 5000); QTRY_VERIFY_WITH_TIMEOUT(p.playing(), 5000);
        p.toggleLoop(); p.stop();

        p.renameTape("Test Mix"); p.signTape("Shane"); p.noteTape("Rewind before returning."); p.moveTrack(1, 0);
        QCOMPARE(files(), QStringList({"Second.wav", first})); QCOMPARE(index(), 1); QVERIFY(p.tape()["dirty"].toBool());
        p.setCover(QUrl::fromLocalFile(audio));
        QCOMPARE(notices.last()[0].toString(), QString("Drop an image to use as the cover"));
        QString listed;
        for (const auto &row : p.insertOf(0)["file"].toList()) if (row.toList().first().toString() == "File") listed = row.toList().last().toString();
        QCOMPARE(listed, QString("Second.wav"));
        p.openUrls({QUrl::fromLocalFile(second)}, true); QCOMPARE(files().size(), 3);
        // Taking away the track that is up puts the next one under the head.
        p.removeTrack(1); QCOMPARE(files(), QStringList({"Second.wav", "Second.wav"}));
        QTRY_COMPARE(p.filename(), QString("Second.wav")); QTRY_COMPARE(p.duration(), 12000);

        const QString saved = directory.filePath("Test Mix.tape");
        p.exportTape(QUrl::fromLocalFile(saved));
        QTRY_VERIFY_WITH_TIMEOUT(p.progress() < 0 && QFile::exists(saved), 5000);
        QCOMPARE(notices.last()[0].toString(), QString("Saved Test Mix.tape")); QVERIFY(!p.tape()["dirty"].toBool());
        p.load({audio}); QTRY_COMPARE(p.duration(), 12000); QVERIFY(!p.tape()["mixtape"].toBool());
        p.load({saved}, true);
        QTRY_COMPARE_WITH_TIMEOUT(p.tape()["name"].toString(), QString("Test Mix"), 5000);
        QCOMPARE(p.tape()["from"].toString(), QString("Shane")); QCOMPARE(p.tape()["note"].toString(), QString("Rewind before returning."));
        QTRY_COMPARE(p.duration(), 12000); QCOMPARE(files(), QStringList({"Second.wav", "Second.wav"}));
        QVERIFY(p.tape()["tracks"].toList().first().toMap()["path"].toString().startsWith(QDir::tempPath()));
        QDesktopServices::setUrlHandler("file", this, "openedUrl");
        p.openFolder(0); QVERIFY(opened.toLocalFile().startsWith(QDir::tempPath()));
        QDesktopServices::unsetUrlHandler("file");
        p.load({directory.filePath("absent.tape")});
        QTRY_VERIFY_WITH_TIMEOUT(notices.last()[0].toString().contains("absent.tape"), 5000);
        p.playTrack(9); QCOMPARE(notices.last()[0].toString(), QString("No such track"));
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
        QTest::keyClick(w, Qt::Key_S); QVERIFY(p.stopped()); QVERIFY(p.position() >= 5000);
        QTest::keyClick(w, Qt::Key_Comma); QCOMPARE(p.position(), 0);
        QTest::keyClick(w, Qt::Key_Return); QVERIFY(p.position() >= 5000);
        QTest::keyClick(w, Qt::Key_B, Qt::ShiftModifier); QCOMPARE(p.bookmark(), -1);
        QTest::keyClick(w, Qt::Key_L); QVERIFY(p.looping());
        QTest::keyClick(w, Qt::Key_I); QVERIFY(w->property("insertVisible").toBool());
        QCOMPARE(p.insert()["file"].toList().first().toList().last().toString(), QString("WAV"));
        QTest::qWait(700); QVERIFY(w->grabWindow().save("build/insert-preview.png"));
        QTest::keyClick(w, Qt::Key_Escape); QVERIFY(!w->property("insertVisible").toBool());
        // The whole title strip of the label opens the insert, not just the side mark on it.
        auto *badge = w->findChild<QQuickItem *>("insertBadge"); QVERIFY(badge);
        for (const auto &spot : {QPointF(15, 15), QPointF(500, 12)}) {
            QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, badge->mapToScene(spot).toPoint());
            QVERIFY(w->property("insertVisible").toBool());
            QTest::keyClick(w, Qt::Key_I); QVERIFY(!w->property("insertVisible").toBool());
        }
        QTest::keyClick(w, Qt::Key_K); QVERIFY(w->property("helpVisible").toBool());
        QTest::keyClick(w, Qt::Key_Escape); QVERIFY(!w->property("helpVisible").toBool());
        auto *key = w->findChild<QQuickItem *>("playKey"); QVERIFY(key);
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, key->mapToScene(QPointF(50, 40)).toPoint());
        QTRY_VERIFY(p.playing()); p.toggle(); QTRY_VERIFY(!p.playing()); QVERIFY(!p.stopped()); p.seek(0);
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
        // With the head lifted the same key is NEXT: on a single file, back to the start, still stopped.
        p.stop(); QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, point);
        QCOMPARE(p.position(), 0); QVERIFY(p.stopped());
        auto *timeline = w->findChild<QQuickItem *>("timeline"); QVERIFY(timeline);
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, timeline->mapToScene(QPointF(timeline->width() / 2, 9)).toPoint());
        QVERIFY(std::abs(p.position() - 6000) < 200);
        p.toggle(); QTest::qWait(120);
        QVERIFY(w->grabWindow().save("build/playing-preview.png"));
        p.stop();
        // The insert as a mixtape's track listing: pick a row out, retitle the tape, remove a track.
        const QString flip = directory.filePath("Flip side.wav"); QVERIFY(QFile::copy(audio, flip));
        p.load({audio, flip}, true); QTRY_COMPARE(p.duration(), 12000);
        QTest::keyClick(w, Qt::Key_I); QTest::qWait(700);
        auto *card = w->findChild<QQuickItem *>("jcard"); QVERIFY(card);
        card->setProperty("selected", 1);
        QCOMPARE(card->property("trackTitle").toString(), QString("Flip side.wav"));
        p.renameTape("Flip Mix"); QCOMPARE(card->property("title").toString(), QString("Flip Mix"));
        // Signed, with a note, the way a tape for a friend would be.
        p.signTape(" Shane "); p.noteTape("For the drive up.\nSide B is the good one.");
        QCOMPARE(p.tape()["from"].toString(), QString("Shane"));
        QCOMPARE(p.tape()["note"].toString(), QString("For the drive up.\nSide B is the good one."));
        // The cover lifts off the card a little askew, never the same way twice; Esc puts it back first.
        QSet<double> tilts;
        for (int i = 0; i < 6; ++i) {
            QVERIFY(QMetaObject::invokeMethod(card, "zoom"));
            const double tilt = card->property("tilt").toDouble();
            QVERIFY(std::abs(tilt) >= 1 && std::abs(tilt) <= 2.6); tilts << tilt;
        }
        QVERIFY(tilts.size() > 1); QVERIFY(card->property("zoomed").toBool());
        QCOMPARE(card->property("held").toString(), QString("cover"));
        QTest::keyClick(w, Qt::Key_Escape);
        QVERIFY(!card->property("zoomed").toBool()); QVERIFY(w->property("insertVisible").toBool());
        // The note comes out the same way, and a click beside it puts it back.
        QVERIFY(QMetaObject::invokeMethod(card, "lift", Q_ARG(QVariant, "note")));
        QCOMPARE(card->property("held").toString(), QString("note")); QTest::qWait(250);
        QVERIFY(w->grabWindow().save("build/note-preview.png"));
        QTest::mouseClick(w, Qt::LeftButton, Qt::NoModifier, QPoint(12, 12));
        QVERIFY(!card->property("zoomed").toBool()); QVERIFY(w->property("insertVisible").toBool());
        QTest::qWait(50); QVERIFY(w->grabWindow().save("build/mixtape-preview.png"));
        QTest::keyClick(w, Qt::Key_Delete); QCOMPARE(p.tape()["tracks"].toList().size(), 1);
        QCOMPARE(card->property("selected").toInt(), -1);
        QTest::keyClick(w, Qt::Key_I); QVERIFY(!w->property("insertVisible").toBool());
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
