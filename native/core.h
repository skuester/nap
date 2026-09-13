#pragma once
#include <QJsonDocument>
#include <QJsonObject>
extern "C" {
void *nap_core_new();
void nap_core_free(void *core);
char *nap_core_request(void *core, const char *request);
void nap_string_free(char *text);
}

// RAII owns the Rust core; no pointer or string outlives its allocator.
class Core {
public:
    Core() : handle(nap_core_new()) {}
    ~Core() { nap_core_free(handle); }
    Core(const Core &) = delete;
    Core &operator=(const Core &) = delete;
    QJsonObject request(const QJsonObject &value) const {
        const auto json = QJsonDocument(value).toJson(QJsonDocument::Compact);
        char *reply = nap_core_request(handle, json.constData());
        const auto result = QJsonDocument::fromJson(QByteArray(reply)).object();
        nap_string_free(reply);
        return result;
    }
private:
    void *handle;
};
