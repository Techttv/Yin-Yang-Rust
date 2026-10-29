#include <QMenu>
#include <QTranslator>
#include <QtTest>

#include "appsetup.h"
#include "coreclient.h"
#include "fakecore.h"
#include "mainwindow.h"
#include "tray.h"

class TrayTest : public QObject
{
    Q_OBJECT

    static bool called(const FakeCore &core, const QString &call) { return core.calls().contains(call); }

private slots:
    void menuHasThreeActions()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        QCOMPARE(tray.menu()->actions().size(), 3);
    }

    void toggleActionInvertsOnly()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        tray.menu()->actions()[1]->trigger();
        QTRY_VERIFY(called(core, "apply --dark --force"));
        QVERIFY2(!called(core, "toggle"), "toggle would also switch the mode to manual");
    }

    void toggleFromDarkGoesLight()
    {
        FakeCore core;
        core.setStatus(R"({"dark_mode": true, "mode": "sunset to sunrise", "times": null, "location": null})");
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        tray.menu()->actions()[1]->trigger();
        QTRY_VERIFY(called(core, "apply --light --force"));
    }

    void middleClickSameAsToggle()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        tray.onActivated(QSystemTrayIcon::MiddleClick);
        QTRY_VERIFY(called(core, "apply --dark --force"));
        QVERIFY(!called(core, "toggle"));
    }

    void unreadableStateDoesNothing()
    {
        FakeCore core;
        core.writeRaw("status.json", "not json");
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        tray.onActivated(QSystemTrayIcon::MiddleClick);
        QTest::qWait(200);
        for (const QString &call : core.calls())
            QVERIFY2(!call.startsWith("apply"), qPrintable(call));
    }

    void leftClickAndOpenActionShowTheWindow()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        QVERIFY(!window.isVisible());
        tray.onActivated(QSystemTrayIcon::Trigger);
        QVERIFY(window.isVisible());
        window.hide();
        tray.menu()->actions()[0]->trigger();
        QVERIFY(window.isVisible());
    }

    void germanTranslationLoads()
    {
        QCOMPARE(QCoreApplication::translate("systray", "Quit"), QStringLiteral("Quit"));
        QVERIFY(installTranslations(qApp, QLocale(QLocale::German)));
        QCOMPARE(QCoreApplication::translate("systray", "Quit"), QStringLiteral("Beenden"));
        for (auto *translator : qApp->findChildren<QTranslator *>())
            qApp->removeTranslator(translator);
    }

    void menuIsTranslated()
    {
        QVERIFY(installTranslations(qApp, QLocale(QLocale::German)));
        FakeCore core;
        CoreClient client(core.program());
        MainWindow window(&client);
        Tray tray(&client, &window);
        QCOMPARE(tray.menu()->actions()[0]->text(), QStringLiteral("Yin Yang öffnen"));
        QCOMPARE(tray.menu()->actions()[2]->text(), QStringLiteral("Beenden"));
        for (auto *translator : qApp->findChildren<QTranslator *>())
            qApp->removeTranslator(translator);
    }

    void missingTranslationKeepsEnglish()
    {
        QVERIFY(!installTranslations(qApp, QLocale(QLocale::Italian)));
        QCOMPARE(QCoreApplication::translate("systray", "Quit"), QStringLiteral("Quit"));
    }

    void applicationIconIsEmbedded() { QVERIFY(!appIcon().isNull()); }
};

QTEST_MAIN(TrayTest)
#include "tst_tray.moc"
