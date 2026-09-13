#include "player.h"
#include <QAudioBuffer>
#include <QFileInfo>
#include <QFile>
#include <QDir>
#include <QMediaMetaData>
#include <QRegularExpression>
#include <sys/xattr.h>
#include <cerrno>
#include <cstring>
#include <cmath>

Player::Player(QObject *parent) : QObject(parent) {
    media.setAudioOutput(&output);
    media.setAudioBufferOutput(&buffers);
    connect(&media, &QMediaPlayer::positionChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::durationChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::metaDataChanged, this, &Player::changed);
    connect(&media, &QMediaPlayer::playbackStateChanged, this, [this] {
        if (!playing()) { samples.clear(); emit waveChanged(); }
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
    return bits.join("  ·  ");
}
void Player::openFile(const QString &file, bool paused, qint64 start, bool ignore) {
    QFileInfo info(file);
    if (!info.isFile() || !info.isReadable()) { emit notice("Cannot open this file"); return; }
    media.stop();
    // Reopening the same file must still apply the requested start policy.
    media.setSource(QUrl());
    path = info.canonicalFilePath();
    mark = -1;
    char value[64];
    const auto n = getxattr(QFile::encodeName(path).constData(), "user.nap.bookmark", value, sizeof(value));
    if (n > 0) { bool ok; auto v = QByteArray(value, n).toLongLong(&ok); if (ok && v >= 0) mark = v; }
    pending = start >= 0 ? start : (!ignore ? mark : -1);
    startPaused = paused;
    media.setSource(QUrl::fromLocalFile(path));
    emit changed();
    if (start < 0 && !ignore && mark >= 0) emit notice("Opened at your bookmark");
}
void Player::openUrl(const QUrl &url) {
    if (url.isLocalFile()) openFile(url.toLocalFile());
    else emit notice("Choose a local audio file");
}
void Player::toggle() { if (loaded()) { if (playing()) media.pause(); else media.play(); } }
void Player::stop() { media.stop(); media.setPosition(0); }
void Player::seek(qint64 ms) { if (media.isSeekable()) media.setPosition(qBound(qint64(0), ms, duration())); }
void Player::skip(int seconds) { seek(position() + qint64(seconds) * 1000); }
void Player::setVolume(double v) { output.setVolume(qBound(0.0, v, 1.0)); emit changed(); }
void Player::toggleMute() { output.setMuted(!muted()); emit changed(); }
void Player::toggleLoop() { media.setLoops(looping() ? 1 : QMediaPlayer::Infinite); emit changed(); }
void Player::saveBookmark(bool remove) {
    if (!loaded()) return;
    const auto name = QFile::encodeName(path);
    const auto value = QByteArray::number(position());
    const int result = remove ? removexattr(name.constData(), "user.nap.bookmark")
                              : setxattr(name.constData(), "user.nap.bookmark", value.constData(), value.size(), 0);
    if (result < 0 && !(remove && errno == ENODATA)) {
        emit notice("Bookmark failed: " + QString::fromLocal8Bit(strerror(errno))); return;
    }
    mark = remove ? -1 : position(); emit changed();
    emit notice(remove ? "Bookmark removed" : "Bookmark saved on this file");
}
QVariantMap Player::palette() const {
    QVariantMap colors{{"background", "#191d20"}, {"foreground", "#e1dfd5"}, {"accent", "#d5ad73"}};
    const QString home = QDir::homePath();
    const QStringList roots{qEnvironmentVariable("XDG_STATE_HOME", home + "/.local/state"), qEnvironmentVariable("XDG_CONFIG_HOME", home + "/.config")};
    for (const auto &root : roots) {
        QFile f(root + "/omarchy/current/theme/colors.toml");
        if (!f.open(QFile::ReadOnly)) continue;
        const QRegularExpression re("^\\s*(background|foreground|accent)\\s*=\\s*\"(#[0-9a-fA-F]{6})\"");
        for (const auto &line : QString::fromUtf8(f.readAll()).split('\n')) {
            auto match = re.match(line); if (match.hasMatch()) colors[match.captured(1)] = match.captured(2);
        }
        break;
    }
    return colors;
}
