#pragma once
#include <QString>
#include <QStringList>
#include <cmath>
#include <optional>

inline std::optional<qint64> timestamp(const QString &text) {
    const auto parts = text.split(':');
    if (parts.isEmpty() || parts.size() > 3) return {};
    double seconds = 0;
    for (int i = 0; i < parts.size(); ++i) {
        bool ok = false;
        const double n = parts[i].toDouble(&ok);
        if (!ok || !std::isfinite(n) || n < 0 || (i > 0 && n >= 60)
            || (i < parts.size() - 1 && std::floor(n) != n)) return {};
        seconds = seconds * 60 + n;
    }
    if (seconds > 1e12) return {};
    return qRound64(seconds * 1000);
}
