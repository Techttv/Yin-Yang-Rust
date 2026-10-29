#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QObject>
#include <QString>

class QProcess;

// Talks to the `yinyang` command line program. The GUI never reads or writes the configuration
// itself: the core owns the schema, the files and the timer.
class CoreClient : public QObject
{
    Q_OBJECT

public:
    enum Target { Auto, Light, Dark };

    explicit CoreClient(QString program, QObject *parent = nullptr);
    ~CoreClient() override;

    // Synchronous, each gives up after 10 seconds. On failure they return an empty value and
    // lastError() says why.
    QJsonArray plugins();
    QJsonObject config();
    QJsonObject defaultConfig();
    QJsonObject status();
    bool save(const QJsonObject &config, QString *error);
    QString lastError() const { return m_lastError; }

    // Asynchronous: they return at once and finished() reports the outcome.
    void apply(Target target, bool force);
    void toggle();

signals:
    void finished(bool ok, QString error);

private:
    struct Result {
        bool ok = false;
        QByteArray out;
        QString error;
    };

    Result run(const QStringList &args, const QByteArray &input = {});
    QJsonDocument query(const QStringList &args);
    void runAsync(const QStringList &args);

    QString m_program;
    QString m_lastError;
};
