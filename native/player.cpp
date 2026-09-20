#include "player.h"
#include <QAudioBuffer>
#include <QDesktopServices>
#include <QFileInfo>
#include <QJsonArray>
#include <QMediaMetaData>
#include <vector>

static constexpr int BandCount = 24, AnalysisFrames = 2048;

Player::Player(QObject *parent) : QObject(parent) {
    scene = core.request({{"op", "visualizer"}})["name"].toString();
    media.setAudioOutput(&output);
    poller.setInterval(40);
    connect(&poller, &QTimer::timeout, this, &Player::poll);
    media.setAudioBufferOutput(&buffers);
    connect(&media, &QMediaPlayer::positionChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::durationChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::metaDataChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::playbackStateChanged, this, [this] {
        if (!playing()) { samples.clear(); bands.clear(); needles.clear(); emit waveChanged(); }
        // A track ran out under a playing head. The core says what is next; at the end of the tape
        // the head lifts, as a deck's auto-stop would.
        if (media.playbackState() == QMediaPlayer::StoppedState && deck == "playing") {
            const auto next = core.request({{"op", "ended"}, {"looping", repeat}});
            if (!next["play"].toBool()) drive("ended");
            if (trackCount() > 1) cue(next, next["play"].toBool());
        }
        emit changed();
    });
    connect(&media, &QMediaPlayer::errorOccurred, this, [this](QMediaPlayer::Error, const QString &s) {
        emit notice("Cannot play: " + s); emit changed();
    });
    connect(&media, &QMediaPlayer::mediaStatusChanged, this, [this](QMediaPlayer::MediaStatus s) {
        // Seeking back from the end reports LoadedMedia again; only the first one is a fresh tape.
        if (s == QMediaPlayer::LoadedMedia && loading) {
            loading = false;
            if (pending >= 0) seek(pending);
            pending = -1;
            // A track found with the head lifted stays that way.
            if (!lifted) drive("loaded", startPaused);
        }
        emit changed();
    });
    connect(&buffers, &QAudioBufferOutput::audioBufferReceived, this, [this](const QAudioBuffer &b) {
        if (!playing() || !b.isValid() || b.frameCount() == 0) return;
        samples.clear();
        const auto format = b.format();
        const auto *data = b.constData<char>();
        // Average channels at evenly spaced frames: a true time-domain waveform.
        for (int i = 0; i < 96; ++i) {
            const int frame = i * (b.frameCount() - 1) / 95;
            float value = 0;
            for (int c = 0; c < format.channelCount(); ++c)
                value += format.normalizedSampleValue(data + frame * format.bytesPerFrame() + c * format.bytesPerSample());
            samples.append(value / format.channelCount());
        }
        // The spectrum and VU meters need contiguous audio, so they get the buffer's latest frames per channel.
        const int frames = qMin(int(b.frameCount()), AnalysisFrames), first = b.frameCount() - frames;
        const int rightChannel = format.channelCount() > 1 ? 1 : 0;
        std::vector<float> left(frames), right(frames);
        for (int i = 0; i < frames; ++i) {
            const auto *frame = data + (first + i) * format.bytesPerFrame();
            left[i] = format.normalizedSampleValue(frame);
            right[i] = format.normalizedSampleValue(frame + rightChannel * format.bytesPerSample());
        }
        float heights[BandCount] = {}, deflection[4] = {};
        nap_analyze(left.data(), right.data(), frames, format.sampleRate(), heights, BandCount, deflection);
        bands.clear(); needles.clear();
        for (float height : heights) bands.append(height);
        for (float needle : deflection) needles.append(needle);
        emit waveChanged();
    });
}

QString Player::filename() const { return path.isEmpty() ? "Your next good listen." : QFileInfo(path).fileName(); }
QString Player::detail() const {
    const auto md = media.metaData();
    QString artist = md.stringValue(QMediaMetaData::ContributingArtist);
    QString album = md.stringValue(QMediaMetaData::AlbumTitle);
    QStringList bits;
    if (!artist.isEmpty()) bits << artist;
    if (!album.isEmpty()) bits << album;
    if (bits.isEmpty() && !path.isEmpty()) bits << QFileInfo(path).suffix().toUpper() << "LOCAL AUDIO";
    if (!size.isEmpty()) bits << size;
    return bits.join("  ·  ");
}
void Player::openFile(const QString &file, bool paused, qint64 start, bool ignore, bool headLifted) {
    const auto result = core.request({{"op", "open"}, {"path", file}, {"start", start}, {"ignore", ignore}});
    if (result.contains("error")) { emit notice(result["error"].toString()); return; }
    media.stop();
    media.setSource(QUrl());
    path = result["path"].toString();
    size = result["size"].toString();
    mark = result["mark"].toInteger(-1);
    pending = result["pending"].toInteger(-1);
    startPaused = paused;
    lifted = headLifted;
    loading = true;
    deck = "stopped"; // changing tapes lifts the head
    // A track inside a tape plays from where it lies in the archive; nothing is unpacked.
    const auto span = result["held"].toObject();
    held.reset(span.isEmpty() ? nullptr : new Stretch(span["archive"].toString(), span["offset"].toInteger(), span["length"].toInteger()));
    if (!held) media.setSource(QUrl::fromLocalFile(path));
    else if (held->open(QIODevice::ReadOnly)) media.setSourceDevice(held.get(), QUrl::fromLocalFile(path));
    else emit notice("Cannot open " + span["archive"].toString());
    card.clear();
    emit changed();
    emit insertChanged();
    if (!result["notice"].toString().isEmpty()) emit notice(result["notice"].toString());
}
void Player::openUrl(const QUrl &url) { openUrls({url}); }
// Dropped or chosen files become the tape in the deck, or with `append` join the one already there.
void Player::openUrls(const QList<QUrl> &urls, bool append) {
    QStringList paths;
    for (const auto &url : urls) if (url.isLocalFile()) paths << url.toLocalFile();
    if (paths.isEmpty()) { emit notice("Choose a local audio file"); return; }
    if (!append) { load(paths); return; }
    edit({{"op", "load"}, {"append", true}, {"paths", QJsonArray::fromStringList(paths)}});
}
// Unsaved work is given up only by asking twice; the core keeps count.
bool Player::mayDiscard(const char *action) {
    const auto verdict = core.request({{"op", "discard"}, {"action", action}});
    if (!verdict["allowed"].toBool()) emit notice(verdict["notice"].toString());
    return verdict["allowed"].toBool();
}
void Player::load(const QStringList &paths, bool paused, qint64 start, bool ignore) {
    if (!mayDiscard("open")) return;
    const auto result = core.request({{"op", "load"}, {"paths", QJsonArray::fromStringList(paths)}});
    if (result.contains("error")) { emit notice(result["error"].toString()); return; }
    refreshTape();
    openFile(reel["tracks"].toList().value(0).toMap()["path"].toString(), paused, start, ignore || trackCount() > 1);
    if (!result["notice"].toString().isEmpty()) emit notice(result["notice"].toString());
}
void Player::refreshTape() {
    reel = core.request({{"op", "tape"}}).toVariantMap();
    // A cover held inside the tape comes over whole, so it is fetched only when it changes.
    const auto cover = reel["cover"].toString();
    if (cover != coverPath) {
        coverPath = cover;
        coverUrl = cover.isEmpty() ? QUrl() : reel["coverHeld"].toBool() ? QUrl(core.request({{"op", "cover"}})["url"].toString()) : QUrl::fromLocalFile(cover);
    }
    reel["coverUrl"] = coverUrl;
    media.setLoops(repeat && trackCount() < 2 ? QMediaPlayer::Infinite : 1);
    emit tapeChanged();
}
// An archive is packed on a core thread; this reports how far along it is.
void Player::poll() {
    const auto report = core.request({{"op", "job"}});
    if (report["active"].toBool()) {
        jobLabel = report["label"].toString();
        fraction = qBound(0.0, report["done"].toDouble() / qMax(1.0, report["total"].toDouble()), 1.0);
        emit progressChanged();
        return;
    }
    poller.stop(); fraction = -1; emit progressChanged();
    if (!report["notice"].toString().isEmpty()) emit notice(report["notice"].toString());
    refreshTape();
}
// Bring the track the core landed on under the head, playing or not.
void Player::cue(const QJsonObject &landed, bool play) {
    const auto track = landed["path"].toString();
    if (track.isEmpty() || track == path) { seek(0); if (play && !playing()) drive("play"); }
    else openFile(track, !play, -1, true, !play && deck == "stopped");
    refreshTape();
}
void Player::playTrack(int index) {
    const auto landed = core.request({{"op", "select"}, {"index", index}});
    if (landed.contains("error")) { emit notice(landed["error"].toString()); return; }
    cue(landed, true);
}
void Player::edit(QJsonObject request) {
    const auto result = core.request(request);
    if (result.contains("error")) { emit notice(result["error"].toString()); return; }
    // Removing the track that is up puts the next one under the head.
    if (result["replaced"].toBool()) cue(result, deck == "playing");
    refreshTape();
}
void Player::moveTrack(int from, int to) { edit({{"op", "edit"}, {"action", "move"}, {"from", from}, {"to", to}}); }
void Player::removeTrack(int index) { edit({{"op", "edit"}, {"action", "remove"}, {"index", index}}); }
void Player::setCover(const QUrl &image) { edit({{"op", "edit"}, {"action", "cover"}, {"path", image.toLocalFile()}}); }
void Player::exportTape(const QUrl &destination) {
    const auto result = core.request({{"op", "export"}, {"dest", destination.toLocalFile()}});
    if (result.contains("error")) { emit notice(result["error"].toString()); return; }
    poller.start(); poll();
}
QVariantMap Player::insertOf(int index) { return core.request({{"op", "insert"}, {"index", index}}).toVariantMap(); }
// The Rust core decides what each key does to the head; this only carries it out.
void Player::drive(const char *event, bool waiting) {
    deck = core.request({{"op", "transport"}, {"state", deck}, {"event", event}, {"waiting", waiting}})["state"].toString();
    if (deck == "playing") media.play();
    else if (playing()) media.pause();
    emit changed();
}
void Player::toggle() { if (loaded()) drive("play"); }
void Player::stop() { if (loaded()) drive("stop"); }
// PREV and NEXT find the start of a track. A single file is a tape of one track.
void Player::search(bool forward) {
    if (!loaded()) return;
    cue(core.request({{"op", "track"}, {"forward", forward}, {"position", position()}}), deck == "playing");
}
void Player::seek(qint64 ms) { if (media.isSeekable()) media.setPosition(core.request({{"op", "seek"}, {"position", ms}, {"duration", duration()}})["position"].toInteger()); }
void Player::skip(int seconds) { seek(core.request({{"op", "skip"}, {"position", position()}, {"seconds", seconds}, {"duration", duration()}})["position"].toInteger()); }
void Player::setVolume(double v) { output.setVolume(core.request({{"op", "volume"}, {"volume", v}})["volume"].toDouble()); emit changed(); }
void Player::toggleMute() { output.setMuted(!muted()); emit changed(); }
// A single file loops seamlessly inside Qt; a tape loops by starting over after its last track.
void Player::toggleLoop() { repeat = !repeat; media.setLoops(repeat && trackCount() < 2 ? QMediaPlayer::Infinite : 1); emit changed(); }
void Player::saveBookmark(bool remove) {
    if (!loaded()) return;
    const auto result = core.request({{"op", "bookmark"}, {"position", position()}, {"remove", remove}});
    if (result.contains("error")) { emit notice("Bookmark failed: " + result["error"].toString()); return; }
    mark = result["mark"].toInteger(-1); emit changed();
    emit notice(result["notice"].toString());
}
void Player::cycleVisualizer() {
    const auto result = core.request({{"op", "visualizer"}, {"next", true}, {"current", scene}});
    scene = result["name"].toString();
    emit visualizerChanged();
    if (!result["notice"].toString().isEmpty()) emit notice(result["notice"].toString());
}
// The folder of a track in the listing, or of the one that is up.
void Player::openFolder(int track) {
    // The core knows where a listed track really is: one held inside a tape is wherever the tape is.
    const auto tracks = reel["tracks"].toList();
    const auto listed = tracks.value(track < 0 ? reel["index"].toInt() : track).toMap();
    if (listed.isEmpty() && path.isEmpty()) return;
    const auto folder = listed.isEmpty() ? QFileInfo(path).absolutePath() : listed["folder"].toString();
    if (!QDesktopServices::openUrl(QUrl::fromLocalFile(folder))) emit notice("Cannot open " + folder);
}
// The insert is read on first look, not on open: the cover can be large and most plays never unfold it.
QVariantMap Player::insert() {
    if (card.isEmpty() && loaded()) card = core.request({{"op", "insert"}}).toVariantMap();
    return card;
}
QVariantMap Player::palette() const {
    return core.request({{"op", "theme"}}).toVariantMap();
}
