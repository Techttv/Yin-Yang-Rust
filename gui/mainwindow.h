#pragma once

#include <QJsonObject>
#include <QList>
#include <QMainWindow>
#include <memory>

class CoreClient;
class QAbstractButton;
class QGroupBox;
class QJsonArray;

namespace Ui {
class main_window;
}

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(CoreClient *client, QWidget *parent = nullptr);
    ~MainWindow() override;

    bool hasUnsavedChanges() const { return m_dirty; }

protected:
    void closeEvent(QCloseEvent *event) override;

private:
    // The two editors of one plugin: combo boxes (choice) or line edits (text, file).
    struct PluginRow {
        QString key;
        QGroupBox *group = nullptr;
        QWidget *light = nullptr;
        QWidget *dark = nullptr;
        int themeCount = 0;
    };

    void buildPlugins(const QJsonArray &plugins);
    void load();
    void fill(const QJsonObject &config);
    void fillTimesAndLocation(const QJsonObject &status);
    void restoreDefaults();
    QJsonObject collect() const;
    bool saveAndApply();
    void setBusy(bool busy);
    void applied(bool ok, const QString &error);
    void reportError(const QString &text);
    void chooseFile(QWidget *edit, bool dark);
    void onEdited();
    void updateVisibility();
    void updateLabel();
    bool isManual() const;

    CoreClient *m_client;
    std::unique_ptr<Ui::main_window> ui;
    QJsonObject m_config;
    QList<PluginRow> m_rows;
    bool m_loading = false;
    bool m_dirty = false;
    bool m_sunTimesKnown = false;
};
