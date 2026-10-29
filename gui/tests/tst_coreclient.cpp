#include <QElapsedTimer>
#include <QSignalSpy>
#include <QtTest>

#include "coreclient.h"
#include "fakecore.h"

class CoreClientTest : public QObject
{
    Q_OBJECT

private slots:
    void parsesPlugins()
    {
        FakeCore core;
        CoreClient client(core.program());
        const QJsonArray plugins = client.plugins();
        QCOMPARE(plugins.size(), 3);
        QCOMPARE(plugins[0].toObject()["key"].toString(), QStringLiteral("system"));
        QCOMPARE(plugins[0].toObject()["themes"].toArray().size(), 2);
    }

    void parsesConfigAndStatus()
    {
        FakeCore core;
        CoreClient client(core.program());
        QCOMPARE(client.config()["mode"].toString(), QStringLiteral("sunset to sunrise"));
        QCOMPARE(client.status()["times"].toArray()[0].toString(), QStringLiteral("06:12"));
    }

    void asksForTheDefaultConfig()
    {
        FakeCore core;
        CoreClient client(core.program());
        QCOMPARE(client.defaultConfig()["mode"].toString(), QStringLiteral("manual"));
        QCOMPARE(core.calls().last(), QStringLiteral("config show --defaults"));
    }

    void saveWritesStdin()
    {
        FakeCore core;
        CoreClient client(core.program());
        QJsonObject cfg = core.config();
        cfg["boot_offset"] = 42;
        QString err;
        QVERIFY(client.save(cfg, &err));
        QCOMPARE(core.saved(), cfg);
        QVERIFY(core.calls().contains(QStringLiteral("config save")));
    }

    void failureReported()
    {
        FakeCore core;
        qputenv("FAKE_SAVE_EXIT", "2");
        CoreClient client(core.program());
        QString err;
        QVERIFY(!client.save(core.config(), &err));
        QVERIFY(err.contains(QStringLiteral("boom")));
    }

    void missingProgramIsReported()
    {
        CoreClient client(QStringLiteral("/nonexistent/yinyang"));
        QVERIFY(client.config().isEmpty());
        QVERIFY(!client.lastError().isEmpty());
        QString err;
        QVERIFY(!client.save(QJsonObject(), &err));
        QVERIFY(!err.isEmpty());
    }

    void garbageAnswerIsReported()
    {
        FakeCore core;
        core.writeRaw("config.json", "not json");
        CoreClient client(core.program());
        QVERIFY(client.config().isEmpty());
        QVERIFY(!client.lastError().isEmpty());
    }

    void applyIsAsync()
    {
        FakeCore core;
        qputenv("FAKE_SLEEP", "0.4");
        CoreClient client(core.program());
        QSignalSpy spy(&client, &CoreClient::finished);

        QElapsedTimer timer;
        timer.start();
        client.apply(CoreClient::Auto, true);
        QVERIFY2(timer.elapsed() < 300, "apply() must return before the program is done");
        QCOMPARE(spy.size(), 0);

        QVERIFY(spy.wait(5000));
        QCOMPARE(spy.takeFirst().at(0).toBool(), true);
        QCOMPARE(core.calls().last(), QStringLiteral("apply --auto --force"));
    }

    void applyArguments()
    {
        FakeCore core;
        CoreClient client(core.program());
        QSignalSpy spy(&client, &CoreClient::finished);
        client.apply(CoreClient::Dark, false);
        QVERIFY(spy.wait(5000));
        client.apply(CoreClient::Light, true);
        QVERIFY(spy.wait(5000));
        client.toggle();
        QVERIFY(spy.wait(5000));
        QCOMPARE(core.calls().mid(0, 3),
                 (QStringList{"apply --dark", "apply --light --force", "toggle"}));
    }

    void applyFailureCarriesTheProgramsMessage()
    {
        FakeCore core;
        qputenv("FAKE_APPLY_EXIT", "1");
        CoreClient client(core.program());
        QSignalSpy spy(&client, &CoreClient::finished);
        client.apply(CoreClient::Auto, true);
        QVERIFY(spy.wait(5000));
        const QList<QVariant> args = spy.takeFirst();
        QCOMPARE(args.at(0).toBool(), false);
        QVERIFY(args.at(1).toString().contains(QStringLiteral("no way")));
    }

    void applyWithMissingProgramFinishesOnce()
    {
        CoreClient client(QStringLiteral("/nonexistent/yinyang"));
        QSignalSpy spy(&client, &CoreClient::finished);
        client.toggle();
        QVERIFY(spy.wait(5000));
        QTest::qWait(100);
        QCOMPARE(spy.size(), 1);
        QCOMPARE(spy.at(0).at(0).toBool(), false);
    }
};

QTEST_MAIN(CoreClientTest)
#include "tst_coreclient.moc"
