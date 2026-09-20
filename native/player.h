#pragma once
#include <QObject>
#include "core.h"
#include <QMediaPlayer>
#include <QAudioOutput>
#include <QAudioBufferOutput>
#include <QFile>
#include <memory>
#include <QVariantList>
#include <QVariantMap>
#include <QTimer>
#include <QUrl>

// A track held inside a .tape: that stretch of the archive, read as though it were a file of
// its own. A tape is an uncompressed tar, so the track's bytes lie there whole and in order.
class Stretch : public QIODevice {
public:
    Stretch(const QString &archive, qint64 offset, qint64 length) : file(archive), start(offset), length(length) {}
    bool open(OpenMode mode) override { return file.open(ReadOnly) && file.seek(start) && QIODevice::open(mode); }
    void close() override { file.close(); QIODevice::close(); }
    bool isSequential() const override { return false; }
    qint64 size() const override { return length; }
    bool seek(qint64 to) override { return to >= 0 && to <= length && QIODevice::seek(to) && file.seek(start + to); }
protected:
    qint64 readData(char *data, qint64 most) override { return file.read(data, qMin(most, start + length - file.pos())); }
    qint64 writeData(const char *, qint64) override { return -1; }
private:
    QFile file;
    qint64 start, length;
};

class Player : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString filename READ filename NOTIFY changed)
    Q_PROPERTY(QString detail READ detail NOTIFY changed)
    Q_PROPERTY(qint64 position READ position NOTIFY changed)
    Q_PROPERTY(qint64 duration READ duration NOTIFY changed)
    Q_PROPERTY(qint64 bookmark READ bookmark NOTIFY changed)
    Q_PROPERTY(bool playing READ playing NOTIFY changed)
    Q_PROPERTY(bool stopped READ stopped NOTIFY changed)
    Q_PROPERTY(bool loaded READ loaded NOTIFY changed)
    Q_PROPERTY(bool looping READ looping NOTIFY changed)
    Q_PROPERTY(bool muted READ muted NOTIFY changed)
    Q_PROPERTY(double volume READ volume NOTIFY changed)
    Q_PROPERTY(QVariantList wave READ wave NOTIFY waveChanged)
    Q_PROPERTY(QVariantList spectrum READ spectrum NOTIFY waveChanged)
    Q_PROPERTY(QVariantList levels READ levels NOTIFY waveChanged)
    Q_PROPERTY(QString visualizer READ visualizer NOTIFY visualizerChanged)
    Q_PROPERTY(QVariantMap palette READ palette CONSTANT)
    Q_PROPERTY(QVariantMap insert READ insert NOTIFY insertChanged)
    Q_PROPERTY(QVariantMap tape READ tape NOTIFY tapeChanged)
    Q_PROPERTY(double progress READ progress NOTIFY progressChanged)
    Q_PROPERTY(QString progressLabel READ progressLabel NOTIFY progressChanged)
public:
    explicit Player(QObject *parent = nullptr);
    QString filename() const;
    QString detail() const;
    qint64 position() const { return media.position(); }
    qint64 duration() const { return media.duration(); }
    qint64 bookmark() const { return mark; }
    bool playing() const { return media.playbackState() == QMediaPlayer::PlayingState; }
    bool loaded() const { return !media.source().isEmpty(); }
    bool stopped() const { return deck == "stopped"; }
    bool looping() const { return repeat; }
    bool muted() const { return output.isMuted(); }
    double volume() const { return output.volume(); }
    QVariantList wave() const { return samples; }
    QVariantList spectrum() const { return bands; }
    QVariantList levels() const { return needles; }
    QString visualizer() const { return scene; }
    QVariantMap palette() const;
    QVariantMap insert();
    QVariantMap tape() const { return reel; }
    double progress() const { return fraction; }
    QString progressLabel() const { return jobLabel; }
    void load(const QStringList &paths, bool paused = false, qint64 start = -1, bool ignore = false);
    void openFile(const QString &path, bool paused = false, qint64 start = -1, bool ignore = false, bool lifted = false);
    Q_INVOKABLE void openUrl(const QUrl &url);
    Q_INVOKABLE void openUrls(const QList<QUrl> &urls, bool append = false);
    Q_INVOKABLE void playTrack(int index);
    Q_INVOKABLE void moveTrack(int from, int to);
    Q_INVOKABLE void removeTrack(int index);
    // Start side B at a track; asked of the track it already starts at, go back to one side.
    Q_INVOKABLE void turnAt(int index) { edit({{"op", "edit"}, {"action", "side"}, {"index", index}}); }
    Q_INVOKABLE void flip();
    Q_INVOKABLE void renameTape(const QString &name) { writeOnTape("name", name); }
    Q_INVOKABLE void signTape(const QString &from) { writeOnTape("from", from); }
    Q_INVOKABLE void noteTape(const QString &note) { writeOnTape("note", note); }
    Q_INVOKABLE void setCover(const QUrl &image);
    Q_INVOKABLE void exportTape(const QUrl &destination);
    // False, with a notice, the first time quitting would discard an unsaved tape.
    Q_INVOKABLE bool mayQuit() { return mayDiscard("quit"); }
    Q_INVOKABLE QVariantMap insertOf(int index);
    Q_INVOKABLE void toggle();
    Q_INVOKABLE void stop();
    Q_INVOKABLE void previous() { search(false); }
    Q_INVOKABLE void next() { search(true); }
    Q_INVOKABLE void seek(qint64 ms);
    Q_INVOKABLE void skip(int seconds);
    Q_INVOKABLE void setVolume(double value);
    Q_INVOKABLE void toggleMute();
    Q_INVOKABLE void toggleLoop();
    Q_INVOKABLE void saveBookmark(bool remove = false);
    Q_INVOKABLE void returnToBookmark();
    Q_INVOKABLE void openFolder(int track = -1);
    Q_INVOKABLE void cycleVisualizer();
signals:
    void changed();
    void waveChanged();
    void insertChanged();
    void visualizerChanged();
    void tapeChanged();
    void progressChanged();
    void notice(const QString &message);
private:
    void drive(const char *event, bool waiting = false);
    void search(bool forward);
    bool mayDiscard(const char *action);
    void cue(const QJsonObject &landed, bool play);
    void edit(QJsonObject request);
    void writeOnTape(const char *field, const QString &text) { edit({{"op", "edit"}, {"action", field}, {"text", text}}); }
    void refreshTape();
    void poll();
    int trackCount() const { return reel["tracks"].toList().size(); }
    Core core;
    // Declared before the player, so it outlives it: the player reads a held track through this.
    std::unique_ptr<Stretch> held;
    QMediaPlayer media;
    QAudioOutput output;
    QAudioBufferOutput buffers;
    QVariantList samples, bands, needles;
    QString scene, deck = "stopped";
    QVariantMap card, reel;
    QString coverPath;
    QUrl coverUrl;
    QTimer poller;
    QString jobLabel;
    double fraction = -1;
    bool repeat = false, lifted = false;
    QString path, size;
    qint64 mark = -1, pending = -1;
    bool startPaused = false, loading = false;
};
