#include "coreclient.h"

#include <QJsonDocument>
#include <QProcess>

namespace {
constexpr int TimeoutMs = 10'000;

QString describe(QProcess &process, const QString &program)
{
    if (process.error() == QProcess::FailedToStart)
        return QStringLiteral("cannot start %1: %2").arg(program, process.errorString());
    if (process.error() == QProcess::Crashed && process.exitStatus() == QProcess::CrashExit)
        return QStringLiteral("%1 crashed").arg(program);
    const QString text = QString::fromUtf8(process.readAllStandardError()).trimmed();
    return text.isEmpty() ? QStringLiteral("%1 exited with status %2").arg(program).arg(process.exitCode())
                          : text;
}
}

CoreClient::CoreClient(QString program, QObject *parent)
    : QObject(parent)
    , m_program(std::move(program))
{
}

CoreClient::~CoreClient()
{
    // A switch that is half done is worse than a short wait when the window closes.
    for (auto *process : findChildren<QProcess *>()) {
        if (process->state() != QProcess::NotRunning && !process->waitForFinished(15'000))
            process->kill();
    }
}

CoreClient::Result CoreClient::run(const QStringList &args, const QByteArray &input)
{
    QProcess process;
    process.start(m_program, args);
    if (!process.waitForStarted(TimeoutMs))
        return {false, {}, describe(process, m_program)};
    process.write(input);
    process.closeWriteChannel();
    if (!process.waitForFinished(TimeoutMs)) {
        process.kill();
        process.waitForFinished();
        return {false, {}, QStringLiteral("%1 did not answer in %2 s").arg(m_program).arg(TimeoutMs / 1000)};
    }
    const bool ok = process.exitStatus() == QProcess::NormalExit && process.exitCode() == 0;
    return {ok, process.readAllStandardOutput(), ok ? QString() : describe(process, m_program)};
}

QJsonDocument CoreClient::query(const QStringList &args)
{
    const Result result = run(args);
    if (!result.ok) {
        m_lastError = result.error;
        return {};
    }
    QJsonParseError parseError;
    const QJsonDocument doc = QJsonDocument::fromJson(result.out, &parseError);
    if (parseError.error != QJsonParseError::NoError) {
        m_lastError = QStringLiteral("%1 %2 gave an unreadable answer: %3")
                          .arg(m_program, args.join(' '), parseError.errorString());
        return {};
    }
    m_lastError.clear();
    return doc;
}

QJsonArray CoreClient::plugins()
{
    return query({"plugins", "--json"}).array();
}

QJsonObject CoreClient::config()
{
    return query({"config", "show"}).object();
}

QJsonObject CoreClient::defaultConfig()
{
    return query({"config", "show", "--defaults"}).object();
}

QJsonObject CoreClient::status()
{
    return query({"status"}).object();
}

bool CoreClient::save(const QJsonObject &config, QString *error)
{
    const Result result = run({"config", "save"}, QJsonDocument(config).toJson(QJsonDocument::Compact));
    if (!result.ok && error)
        *error = result.error;
    return result.ok;
}

void CoreClient::apply(Target target, bool force)
{
    QStringList args{"apply"};
    args << (target == Auto ? "--auto" : target == Dark ? "--dark" : "--light");
    if (force)
        args << "--force";
    runAsync(args);
}

void CoreClient::toggle()
{
    runAsync({"toggle"});
}

void CoreClient::runAsync(const QStringList &args)
{
    auto *process = new QProcess(this);
    // errorOccurred(FailedToStart) comes without finished(); a crash comes with both.
    connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart) {
            emit finished(false, describe(*process, m_program));
            process->deleteLater();
        }
    });
    connect(process, &QProcess::finished, this, [this, process](int code, QProcess::ExitStatus status) {
        const bool ok = status == QProcess::NormalExit && code == 0;
        emit finished(ok, ok ? QString() : describe(*process, m_program));
        process->deleteLater();
    });
    process->start(m_program, args);
}
