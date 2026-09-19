#include "player.h"
#include <QAudioBuffer>
#include <QDesktopServices>
#include <QFileInfo>
#include <QMediaMetaData>
#include <vector>

static constexpr int BandCount = 24, AnalysisFrames = 2048;

Player::Player(QObject *parent) : QObject(parent) {
    scene = core.request({{"op", "visualizer"}})["name"].toString();
    media.setAudioOutput(&output);
    media.setAudioBufferOutput(&buffers);
    connect(&media, &QMediaPlayer::positionChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::durationChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::metaDataChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::playbackStateChanged, this, [this] {
        if (!playing()) { samples.clear(); bands.clear(); needles.clear(); emit waveChanged(); }
        emit changed();
    });
    connect(&media, &QMediaPlayer::errorOccurred, this, [this](QMediaPlayer::Error, const QString &s) {
        emit notice("Cannot play: " + s); emit changed();
    });
    connect(&media, &QMediaPlayer::mediaStatusChanged, this, [this](QMediaPlayer::MediaStatus s) {
        if (s == QMediaPlayer::LoadedMedia) {
            if (pending >= 0) seek(pending);
            pending = -1;
            if (!startPaused) media.play();
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
void Player::openFile(const QString &file, bool paused, qint64 start, bool ignore) {
    const auto result = core.request({{"op", "open"}, {"path", file}, {"start", start}, {"ignore", ignore}});
    if (result.contains("error")) { emit notice(result["error"].toString()); return; }
    media.stop();
    media.setSource(QUrl());
    path = result["path"].toString();
    size = result["size"].toString();
    mark = result["mark"].toInteger(-1);
    pending = result["pending"].toInteger(-1);
    startPaused = paused;
    media.setSource(QUrl::fromLocalFile(path));
    card.clear();
    emit changed();
    emit insertChanged();
    if (!result["notice"].toString().isEmpty()) emit notice(result["notice"].toString());
}
void Player::openUrl(const QUrl &url) {
    if (url.isLocalFile()) openFile(url.toLocalFile());
    else emit notice("Choose a local audio file");
}
void Player::toggle() { if (loaded()) { if (playing()) media.pause(); else media.play(); } }
void Player::stop() { media.stop(); media.setPosition(0); }
void Player::seek(qint64 ms) { if (media.isSeekable()) media.setPosition(core.request({{"op", "seek"}, {"position", ms}, {"duration", duration()}})["position"].toInteger()); }
void Player::skip(int seconds) { seek(core.request({{"op", "skip"}, {"position", position()}, {"seconds", seconds}, {"duration", duration()}})["position"].toInteger()); }
void Player::setVolume(double v) { output.setVolume(core.request({{"op", "volume"}, {"volume", v}})["volume"].toDouble()); emit changed(); }
void Player::toggleMute() { output.setMuted(!muted()); emit changed(); }
void Player::toggleLoop() { media.setLoops(looping() ? 1 : QMediaPlayer::Infinite); emit changed(); }
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
void Player::openFolder() {
    if (path.isEmpty()) return;
    const auto folder = QFileInfo(path).absolutePath();
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
