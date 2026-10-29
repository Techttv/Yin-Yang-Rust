#pragma once

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTemporaryDir>

// A directory the fake `yinyang` (fake_yinyang.sh) reads its answers from and logs its calls to.
struct FakeCore {
    QTemporaryDir dir;

    FakeCore()
    {
        for (const char *name : {"FAKE_SAVE_EXIT", "FAKE_APPLY_EXIT", "FAKE_SLEEP"})
            qunsetenv(name);
        qputenv("FAKE_DIR", dir.path().toUtf8());
        write("plugins.json", QJsonDocument::fromJson(R"([
            {"key": "system", "name": "System", "kind": "choice", "available": true,
             "themes": [{"id": "org.kde.breeze.desktop", "label": "Breeze"},
                        {"id": "org.kde.breezedark.desktop", "label": "Breeze Dark"}]},
            {"key": "kvantum", "name": "Kvantum", "kind": "choice", "available": false,
             "themes": []},
            {"key": "custom", "name": "Custom", "kind": "text", "available": true,
             "themes": []}
        ])"));
        setConfig(QJsonDocument::fromJson(R"({
            "version": 4.0, "running": true, "dark_mode": false, "mode": "sunset to sunrise",
            "coordinates": [41.9, 12.5], "update_location": false, "update_interval": 60,
            "times": ["07:00", "20:00"], "boot_offset": 10,
            "plugins": {
                "system": {"enabled": true, "light_theme": "org.kde.breeze.desktop", "dark_theme": "org.kde.breezedark.desktop"},
                "kvantum": {"enabled": true, "light_theme": "KvFlatLight", "dark_theme": "KvFlat"},
                "custom": {"enabled": false, "light_theme": null, "dark_theme": null},
                "notification": {"enabled": true, "light_theme": "Day", "dark_theme": "Night"}
            }})").object());
        write("defaults.json", QJsonDocument::fromJson(R"({
            "version": 4.0, "running": false, "dark_mode": false, "mode": "manual",
            "coordinates": [0.0, 0.0], "update_location": false, "update_interval": 60,
            "times": ["07:00", "20:00"], "boot_offset": 10,
            "plugins": {
                "system": {"enabled": false, "light_theme": "org.kde.breeze.desktop", "dark_theme": "org.kde.breezedark.desktop"},
                "kvantum": {"enabled": false, "light_theme": "KvFlatLight", "dark_theme": "KvFlat"},
                "custom": {"enabled": false, "light_theme": null, "dark_theme": null},
                "notification": {"enabled": false, "light_theme": "Day", "dark_theme": "Night"}
            }})"));
        setStatus(R"({"dark_mode": false, "mode": "sunset to sunrise", "times": ["06:12", "19:48"], "location": [41.9, 12.5]})");
    }

    QString program() const { return QStringLiteral(FAKE_YINYANG); }

    void write(const QString &name, const QJsonDocument &doc) { writeRaw(name, doc.toJson()); }
    void writeRaw(const QString &name, const QByteArray &bytes)
    {
        QFile file(dir.filePath(name));
        if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate))
            qFatal("cannot write %s", qPrintable(file.fileName()));
        file.write(bytes);
    }
    QByteArray read(const QString &name) const
    {
        QFile file(dir.filePath(name));
        return file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
    }

    void setConfig(const QJsonObject &config) { write("config.json", QJsonDocument(config)); }
    void setStatus(const QByteArray &json) { writeRaw("status.json", json); }
    QJsonObject config() const { return QJsonDocument::fromJson(read("config.json")).object(); }
    QJsonObject saved() const { return QJsonDocument::fromJson(read("saved.json")).object(); }
    QStringList calls() const { return QString::fromUtf8(read("calls.log")).split('\n', Qt::SkipEmptyParts); }
};
