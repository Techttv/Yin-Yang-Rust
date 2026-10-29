#include <QCheckBox>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QGroupBox>
#include <QLabel>
#include <QPushButton>
#include <QLineEdit>
#include <QRadioButton>
#include <QSpinBox>
#include <QStatusBar>
#include <QTimeEdit>
#include <QtTest>

#include "coreclient.h"
#include "fakecore.h"
#include "mainwindow.h"

class MainWindowTest : public QObject
{
    Q_OBJECT

    template<class T>
    static T *child(QWidget *w, const char *name)
    {
        T *found = w->findChild<T *>(name);
        if (!found)
            qFatal("no widget %s", name);
        return found;
    }

    static void click(MainWindow &w, QDialogButtonBox::StandardButton which)
    {
        child<QDialogButtonBox>(&w, "btn_box")->button(which)->click();
    }

private slots:
    void buildsPluginGroups()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        auto *group = child<QGroupBox>(&w, "groupSystem");
        QCOMPARE(group->findChildren<QComboBox *>().size(), 2);
        QVERIFY(group->isChecked());
        QCOMPARE(group->findChildren<QComboBox *>()[0]->currentData().toString(),
                 QStringLiteral("org.kde.breeze.desktop"));
        QCOMPARE(group->findChildren<QComboBox *>()[1]->currentText(), QStringLiteral("Breeze Dark"));

        auto *custom = child<QGroupBox>(&w, "groupCustom");
        QCOMPARE(custom->findChildren<QLineEdit *>().size(), 2);
        QVERIFY(!custom->isChecked());
    }

    void unavailablePluginHidden()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        QVERIFY(w.findChild<QGroupBox *>("groupKvantum") == nullptr);
    }

    void notificationFollowsItsCheckbox()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        QVERIFY(child<QCheckBox>(&w, "toggle_notification")->isChecked());
        QVERIFY(w.findChild<QGroupBox *>("groupNotification") == nullptr);
    }

    void unknownCurrentThemeKept()
    {
        FakeCore core;
        QJsonObject cfg = core.config();
        QJsonObject plugins = cfg["plugins"].toObject();
        QJsonObject system = plugins["system"].toObject();
        system["light_theme"] = "Solarized Dark";
        plugins["system"] = system;
        cfg["plugins"] = plugins;
        core.setConfig(cfg);

        CoreClient client(core.program());
        MainWindow w(&client);
        auto *light = child<QGroupBox>(&w, "groupSystem")->findChildren<QComboBox *>()[0];
        QCOMPARE(light->count(), 3);
        QCOMPARE(light->currentText(), QStringLiteral("Solarized Dark"));

        click(w, QDialogButtonBox::Apply);
        QCOMPARE(core.saved()["plugins"].toObject()["system"].toObject()["light_theme"].toString(),
                 QStringLiteral("Solarized Dark"));
    }

    void sunModeShowsComputedTimes()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        QVERIFY(child<QRadioButton>(&w, "btn_sun")->isChecked());
        QCOMPARE(child<QTimeEdit>(&w, "inp_time_light")->time(), QTime(6, 12));
        QCOMPARE(child<QTimeEdit>(&w, "inp_time_dark")->time(), QTime(19, 48));
        const QString label = child<QLabel>(&w, "label_active")->text();
        QVERIFY2(label.contains("19:48") && label.contains("06:12"), qPrintable(label));
    }

    void applyButtonSavesAndApplies()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        click(w, QDialogButtonBox::Apply);

        const QJsonObject saved = core.saved();
        QCOMPARE(saved["mode"].toString(), QStringLiteral("sunset to sunrise"));
        QCOMPARE(saved["version"].toDouble(), 4.0);
        QCOMPARE(saved["update_interval"].toInt(), 60);
        QCOMPARE(saved["plugins"].toObject()["kvantum"].toObject()["enabled"].toBool(), true);
        QTRY_VERIFY(core.calls().contains(QStringLiteral("apply --auto --force")));
        QVERIFY(core.calls().indexOf("config save") < core.calls().indexOf("apply --auto --force"));
        QVERIFY(!w.hasUnsavedChanges());
    }

    void saveReflectsWidgets()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        child<QRadioButton>(&w, "btn_schedule")->setChecked(true);
        child<QTimeEdit>(&w, "inp_time_light")->setTime(QTime(6, 30));
        child<QTimeEdit>(&w, "inp_time_dark")->setTime(QTime(21, 45));
        child<QSpinBox>(&w, "bootOffset")->setValue(25);
        child<QCheckBox>(&w, "toggle_notification")->setChecked(false);
        auto *system = child<QGroupBox>(&w, "groupSystem");
        system->findChildren<QComboBox *>()[0]->setCurrentIndex(1);
        child<QGroupBox>(&w, "groupCustom")->setChecked(true);
        child<QGroupBox>(&w, "groupCustom")->findChildren<QLineEdit *>()[1]->setText("notify-send dark");
        QVERIFY(w.hasUnsavedChanges());

        click(w, QDialogButtonBox::Apply);

        const QJsonObject saved = core.saved();
        QCOMPARE(saved["mode"].toString(), QStringLiteral("manual time"));
        QCOMPARE(saved["times"].toArray(), (QJsonArray{"06:30", "21:45"}));
        QCOMPARE(saved["boot_offset"].toInt(), 25);
        const QJsonObject plugins = saved["plugins"].toObject();
        QCOMPARE(plugins["notification"].toObject()["enabled"].toBool(), false);
        QCOMPARE(plugins["notification"].toObject()["light_theme"].toString(), QStringLiteral("Day"));
        QCOMPARE(plugins["system"].toObject()["light_theme"].toString(),
                 QStringLiteral("org.kde.breezedark.desktop"));
        QCOMPARE(plugins["custom"].toObject()["enabled"].toBool(), true);
        QCOMPARE(plugins["custom"].toObject()["dark_theme"].toString(), QStringLiteral("notify-send dark"));
        QVERIFY(plugins["custom"].toObject()["light_theme"].isNull());
        QTRY_VERIFY(core.calls().contains(QStringLiteral("apply --auto --force")));
    }

    void manualModeApplyOnlySaves()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        child<QCheckBox>(&w, "btn_enable")->setChecked(false);
        click(w, QDialogButtonBox::Apply);
        QCOMPARE(core.saved()["mode"].toString(), QStringLiteral("manual"));
        QTest::qWait(200);
        QVERIFY(!core.calls().contains(QStringLiteral("apply --auto --force")));
    }

    void saveFailureKeepsTheChangesPending()
    {
        FakeCore core;
        qputenv("FAKE_SAVE_EXIT", "2");
        CoreClient client(core.program());
        MainWindow w(&client);
        child<QSpinBox>(&w, "bootOffset")->setValue(25);
        click(w, QDialogButtonBox::Apply);
        QVERIFY(w.hasUnsavedChanges());
        QVERIFY(w.statusBar()->currentMessage().contains(QStringLiteral("boom")));
        QVERIFY(!core.calls().contains(QStringLiteral("apply --auto --force")));
    }

    void lightAndDarkButtonsApplyDirectly()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        child<QPushButton>(&w, "button_dark")->click();
        QTRY_VERIFY(core.calls().contains(QStringLiteral("apply --dark --force")));
        QTRY_VERIFY(child<QPushButton>(&w, "button_light")->isEnabled());
        child<QPushButton>(&w, "button_light")->click();
        QTRY_VERIFY(core.calls().contains(QStringLiteral("apply --light --force")));
    }

    void restoreDefaultsButton()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        auto *box = child<QDialogButtonBox>(&w, "btn_box");
        QVERIFY(box->button(QDialogButtonBox::RestoreDefaults));
        QVERIFY(box->button(QDialogButtonBox::Cancel));

        child<QSpinBox>(&w, "bootOffset")->setValue(99);
        child<QGroupBox>(&w, "groupSystem")->findChildren<QComboBox *>()[1]->setCurrentIndex(0);
        click(w, QDialogButtonBox::RestoreDefaults);

        QCOMPARE(child<QSpinBox>(&w, "bootOffset")->value(), 10);
        QVERIFY(!child<QCheckBox>(&w, "btn_enable")->isChecked());
        QCOMPARE(child<QTimeEdit>(&w, "inp_time_light")->time(), QTime(7, 0));
        QCOMPARE(child<QTimeEdit>(&w, "inp_time_dark")->time(), QTime(20, 0));
        QVERIFY(!child<QCheckBox>(&w, "toggle_notification")->isChecked());
        auto *system = child<QGroupBox>(&w, "groupSystem");
        QVERIFY(!system->isChecked());
        QCOMPARE(system->findChildren<QComboBox *>()[1]->currentData().toString(),
                 QStringLiteral("org.kde.breezedark.desktop"));
        QVERIFY(w.hasUnsavedChanges());
        QVERIFY2(core.saved().isEmpty(), "restoring defaults only fills the widgets; Apply saves");
    }

    void cancelReloadsFromTheConfig()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        child<QSpinBox>(&w, "bootOffset")->setValue(99);
        QVERIFY(w.hasUnsavedChanges());
        click(w, QDialogButtonBox::Cancel);
        QCOMPARE(child<QSpinBox>(&w, "bootOffset")->value(), 10);
        QVERIFY(!w.hasUnsavedChanges());
    }

    void freshWindowHasNoUnsavedChanges()
    {
        FakeCore core;
        CoreClient client(core.program());
        MainWindow w(&client);
        QVERIFY(!w.hasUnsavedChanges());
    }

    void automaticLocationDisablesCoordinateInput()
    {
        FakeCore core;
        QJsonObject cfg = core.config();
        cfg["update_location"] = true;
        core.setConfig(cfg);
        CoreClient client(core.program());
        MainWindow w(&client);
        QVERIFY(child<QCheckBox>(&w, "btn_location")->isChecked());
        QVERIFY(!child<QWidget>(&w, "location_input")->isEnabled());
    }
};

QTEST_MAIN(MainWindowTest)
#include "tst_mainwindow.moc"
