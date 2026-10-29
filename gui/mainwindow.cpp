#include "mainwindow.h"

#include <QCloseEvent>
#include <QComboBox>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QIcon>
#include <QJsonArray>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QStandardPaths>
#include <QStatusBar>
#include <QVBoxLayout>

#include "appsetup.h"
#include "coreclient.h"
#include "ui_main_window.h"

namespace {

const QString ModeManual = QStringLiteral("manual");
const QString ModeScheduled = QStringLiteral("manual time");
const QString ModeSun = QStringLiteral("sunset to sunrise");
const QString TimeFormat = QStringLiteral("HH:mm");

// Shows `value` in a theme editor. A saved theme that is not among the listed ones is added as an
// entry: falling back to the first one would silently replace the user's choice on the next save.
void setEditor(QWidget *editor, const QJsonValue &value, int themeCount)
{
    if (auto *combo = qobject_cast<QComboBox *>(editor)) {
        while (combo->count() > themeCount)
            combo->removeItem(combo->count() - 1);
        const QString id = value.toString();
        if (id.isEmpty()) {
            combo->setCurrentIndex(-1);
            return;
        }
        int index = combo->findData(id);
        if (index < 0) {
            combo->addItem(id, id);
            index = combo->count() - 1;
        }
        combo->setCurrentIndex(index);
    } else if (auto *edit = qobject_cast<QLineEdit *>(editor)) {
        edit->setText(value.toString());
    }
}

// What the editor holds; an empty one leaves a missing (null) theme missing.
QJsonValue editorValue(const QWidget *editor, const QJsonValue &original)
{
    const bool missing = original.isNull() || original.isUndefined();
    if (auto *combo = qobject_cast<const QComboBox *>(editor))
        return combo->currentIndex() < 0 ? original : QJsonValue(combo->currentData().toString());
    if (auto *edit = qobject_cast<const QLineEdit *>(editor))
        return edit->text().isEmpty() && missing ? original : QJsonValue(edit->text());
    return original;
}

QTime parseTime(const QJsonValue &value, QTime fallback)
{
    const QTime time = QTime::fromString(value.toString(), TimeFormat);
    return time.isValid() ? time : fallback;
}

}

MainWindow::MainWindow(CoreClient *client, QWidget *parent)
    : QMainWindow(parent)
    , m_client(client)
    , ui(std::make_unique<Ui::main_window>())
{
    ui->setupUi(this);
    setWindowIcon(appIcon());
    ui->inp_latitude->setDecimals(4);
    ui->inp_longitude->setDecimals(4);
    ui->label_active->setWordWrap(true);

    buildPlugins(m_client->plugins());

    const std::initializer_list<QAbstractButton *> toggles{ui->btn_enable, ui->btn_schedule, ui->btn_sun,
                                                           ui->btn_location, ui->toggle_notification};
    for (QAbstractButton *button : toggles)
        connect(button, &QAbstractButton::toggled, this, &MainWindow::onEdited);
    for (auto *spin : {ui->inp_latitude, ui->inp_longitude})
        connect(spin, &QDoubleSpinBox::valueChanged, this, &MainWindow::onEdited);
    for (auto *edit : {ui->inp_time_light, ui->inp_time_dark}) {
        connect(edit, &QTimeEdit::timeChanged, this, &MainWindow::onEdited);
        connect(edit, &QTimeEdit::timeChanged, this, &MainWindow::updateLabel);
    }
    connect(ui->bootOffset, &QSpinBox::valueChanged, this, &MainWindow::onEdited);

    connect(ui->btn_enable, &QAbstractButton::toggled, this, &MainWindow::updateVisibility);
    connect(ui->btn_sun, &QAbstractButton::toggled, this, &MainWindow::updateVisibility);
    connect(ui->btn_location, &QAbstractButton::toggled, ui->location_input, &QWidget::setDisabled);

    connect(ui->btn_box, &QDialogButtonBox::clicked, this, [this](QAbstractButton *button) {
        switch (ui->btn_box->standardButton(button)) {
        case QDialogButtonBox::Apply:
            saveAndApply();
            break;
        case QDialogButtonBox::Cancel:
            load();
            break;
        case QDialogButtonBox::RestoreDefaults:
            restoreDefaults();
            break;
        default:
            break;
        }
    });
    connect(ui->button_light, &QPushButton::clicked, this, [this] {
        setBusy(true);
        m_client->apply(CoreClient::Light, true);
    });
    connect(ui->button_dark, &QPushButton::clicked, this, [this] {
        setBusy(true);
        m_client->apply(CoreClient::Dark, true);
    });
    connect(m_client, &CoreClient::finished, this, &MainWindow::applied);

    load();
}

MainWindow::~MainWindow() = default;

void MainWindow::buildPlugins(const QJsonArray &plugins)
{
    auto *layout = qobject_cast<QVBoxLayout *>(ui->plugins_scroll_content->layout());
    for (const QJsonValue &value : plugins) {
        const QJsonObject plugin = value.toObject();
        const QString key = plugin["key"].toString();
        // The notification has its own check box on the first tab.
        if (!plugin["available"].toBool() || key == QLatin1String("notification"))
            continue;

        const QString name = plugin["name"].toString();
        const QString kind = plugin["kind"].toString();
        const QJsonArray themes = plugin["themes"].toArray();

        auto *group = new QGroupBox(name, ui->plugins_scroll_content);
        group->setObjectName(QStringLiteral("group") + QString(name).remove(' '));
        group->setCheckable(true);
        auto *form = new QFormLayout(group);

        PluginRow row;
        row.key = key;
        row.group = group;
        row.themeCount = themes.size();

        for (const bool dark : {false, true}) {
            QWidget *editor = nullptr;
            QWidget *shown = nullptr;
            if (kind == QLatin1String("choice")) {
                auto *combo = new QComboBox(group);
                combo->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
                combo->setMinimumContentsLength(14);
                combo->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
                for (const QJsonValue &theme : themes)
                    combo->addItem(theme["label"].toString(), theme["id"].toString());
                combo->setCurrentIndex(-1);
                connect(combo, &QComboBox::currentIndexChanged, this, &MainWindow::onEdited);
                editor = shown = combo;
            } else {
                auto *edit = new QLineEdit(group);
                connect(edit, &QLineEdit::textChanged, this, &MainWindow::onEdited);
                editor = edit;
                shown = edit;
                if (kind == QLatin1String("file")) {
                    shown = new QWidget(group);
                    auto *line = new QHBoxLayout(shown);
                    line->setContentsMargins(0, 0, 0, 0);
                    edit->setParent(shown);
                    auto *choose = new QPushButton(tr("Choose…"), shown);
                    connect(choose, &QPushButton::clicked, this, [this, edit, dark] { chooseFile(edit, dark); });
                    line->addWidget(edit);
                    line->addWidget(choose);
                }
            }
            form->addRow(dark ? tr("Dark:") : tr("Light:"), shown);
            (dark ? row.dark : row.light) = editor;
        }

        connect(group, &QGroupBox::toggled, this, &MainWindow::onEdited);
        layout->addWidget(group);
        m_rows.append(row);
    }
    layout->addStretch();
}

void MainWindow::load()
{
    const QJsonObject config = m_client->config();
    const bool ok = !config.isEmpty();
    ui->tab_widget->setEnabled(ok);
    ui->btn_box->button(QDialogButtonBox::Apply)->setEnabled(ok);
    ui->btn_box->button(QDialogButtonBox::RestoreDefaults)->setEnabled(ok);
    if (!ok) {
        reportError(tr("Cannot read the settings: %1").arg(m_client->lastError()));
        return;
    }
    statusBar()->clearMessage();
    m_config = config;
    fill(config);
    fillTimesAndLocation(m_client->status());
}

// Puts a whole configuration into the widgets. The caller decides what that means for m_dirty.
void MainWindow::fill(const QJsonObject &config)
{
    m_loading = true;

    const QString mode = config["mode"].toString();
    ui->btn_enable->setChecked(mode != ModeManual);
    (mode == ModeSun ? ui->btn_sun : ui->btn_schedule)->setChecked(true);

    const QJsonArray times = config["times"].toArray();
    ui->inp_time_light->setTime(parseTime(times.at(0), QTime(7, 0)));
    ui->inp_time_dark->setTime(parseTime(times.at(1), QTime(20, 0)));

    ui->btn_location->setChecked(config["update_location"].toBool());
    ui->location_input->setDisabled(ui->btn_location->isChecked());
    const QJsonArray coordinates = config["coordinates"].toArray();
    ui->inp_latitude->setValue(coordinates.at(0).toDouble());
    ui->inp_longitude->setValue(coordinates.at(1).toDouble());

    ui->bootOffset->setValue(config["boot_offset"].toInt(10));

    const QJsonObject plugins = config["plugins"].toObject();
    ui->toggle_notification->setChecked(plugins["notification"].toObject()["enabled"].toBool());
    for (const PluginRow &row : std::as_const(m_rows)) {
        const QJsonObject entry = plugins[row.key].toObject();
        row.group->setChecked(entry["enabled"].toBool());
        setEditor(row.light, entry["light_theme"], row.themeCount);
        setEditor(row.dark, entry["dark_theme"], row.themeCount);
    }

    m_sunTimesKnown = false;
    updateVisibility();
    m_loading = false;
    m_dirty = false;
}

// In sunset-to-sunrise mode the times and the position are the core's to work out, not the file's.
void MainWindow::fillTimesAndLocation(const QJsonObject &status)
{
    const bool wasLoading = m_loading;
    m_loading = true;
    if (!isManual() && ui->btn_sun->isChecked()) {
        const QJsonArray times = status["times"].toArray();
        m_sunTimesKnown = times.size() == 2;
        if (m_sunTimesKnown) {
            ui->inp_time_light->setTime(parseTime(times.at(0), ui->inp_time_light->time()));
            ui->inp_time_dark->setTime(parseTime(times.at(1), ui->inp_time_dark->time()));
        }
        const QJsonArray position = status["location"].toArray();
        if (ui->btn_location->isChecked() && position.size() == 2) {
            ui->inp_latitude->setValue(position.at(0).toDouble());
            ui->inp_longitude->setValue(position.at(1).toDouble());
        }
    }
    m_loading = wasLoading;
    updateLabel();
}

void MainWindow::restoreDefaults()
{
    const QJsonObject defaults = m_client->defaultConfig();
    if (defaults.isEmpty()) {
        reportError(tr("Cannot read the default settings: %1").arg(m_client->lastError()));
        return;
    }
    fill(defaults);
    updateLabel();
    m_dirty = true;
}

QJsonObject MainWindow::collect() const
{
    QJsonObject config = m_config;
    config["mode"] = isManual() ? ModeManual : ui->btn_schedule->isChecked() ? ModeScheduled : ModeSun;
    config["coordinates"] = QJsonArray{ui->inp_latitude->value(), ui->inp_longitude->value()};
    config["update_location"] = ui->btn_location->isChecked();
    config["times"] = QJsonArray{ui->inp_time_light->time().toString(TimeFormat),
                                 ui->inp_time_dark->time().toString(TimeFormat)};
    config["boot_offset"] = ui->bootOffset->value();

    // Entries of plugins that have no editor here (unavailable ones, other versions') stay as they were.
    QJsonObject plugins = m_config["plugins"].toObject();
    for (const PluginRow &row : m_rows) {
        QJsonObject entry = plugins[row.key].toObject();
        entry["enabled"] = row.group->isChecked();
        entry["light_theme"] = editorValue(row.light, entry["light_theme"]);
        entry["dark_theme"] = editorValue(row.dark, entry["dark_theme"]);
        plugins[row.key] = entry;
    }
    QJsonObject notification = plugins["notification"].toObject();
    notification["enabled"] = ui->toggle_notification->isChecked();
    plugins["notification"] = notification;
    config["plugins"] = plugins;
    return config;
}

bool MainWindow::saveAndApply()
{
    const QJsonObject config = collect();
    QString error;
    if (!m_client->save(config, &error)) {
        reportError(tr("Cannot save the settings: %1").arg(error));
        return false;
    }
    m_config = config;
    m_dirty = false;

    // Manual mode has no schedule to apply; the core has already dropped the timer when it saved.
    if (isManual()) {
        statusBar()->showMessage(tr("Settings saved."), 5000);
        return true;
    }
    setBusy(true);
    m_client->apply(CoreClient::Auto, true);
    return true;
}

void MainWindow::setBusy(bool busy)
{
    m_busy = busy;
    ui->btn_box->button(QDialogButtonBox::Apply)->setEnabled(!busy);
    ui->button_light->setEnabled(!busy);
    ui->button_dark->setEnabled(!busy);
    if (busy)
        statusBar()->showMessage(tr("Applying the theme…"));
}

void MainWindow::applied(bool ok, const QString &error)
{
    // Not ours: the tray reports its own switches.
    if (!m_busy)
        return;
    setBusy(false);
    if (!ok) {
        reportError(tr("The theme could not be applied: %1").arg(error));
        return;
    }
    statusBar()->showMessage(tr("Done."), 5000);
    if (!m_dirty)
        fillTimesAndLocation(m_client->status());
}

// The message stays in the status bar; the box is not modal so that nothing else waits for it.
void MainWindow::reportError(const QString &text)
{
    statusBar()->showMessage(text);
    auto *box = new QMessageBox(QMessageBox::Warning, QStringLiteral("Yin & Yang"), text, QMessageBox::Ok, this);
    box->setAttribute(Qt::WA_DeleteOnClose);
    box->open();
}

void MainWindow::chooseFile(QWidget *edit, bool dark)
{
    auto *line = qobject_cast<QLineEdit *>(edit);
    QString start = QFileInfo(line->text()).absolutePath();
    if (line->text().isEmpty() || !QFileInfo(start).isDir())
        start = QStandardPaths::writableLocation(QStandardPaths::PicturesLocation);
    const QString file = QFileDialog::getOpenFileName(
        this, dark ? tr("Choose the dark file") : tr("Choose the light file"), start,
        tr("Images (*.png *.jpg *.jpeg *.webp *.bmp);;All files (*)"));
    if (!file.isEmpty())
        line->setText(file);
}

void MainWindow::onEdited()
{
    if (!m_loading)
        m_dirty = true;
}

bool MainWindow::isManual() const
{
    return !ui->btn_enable->isChecked();
}

void MainWindow::updateVisibility()
{
    const bool manual = isManual();
    const bool sun = ui->btn_sun->isChecked();
    ui->schedule_settings->setVisible(!manual);
    ui->manual_buttons->setVisible(manual);
    ui->time->setVisible(!sun);
    ui->location->setVisible(sun);
    ui->label_active->setVisible(!manual);
    updateLabel();
}

void MainWindow::updateLabel()
{
    if (!isManual() && ui->btn_sun->isChecked() && !m_sunTimesKnown) {
        ui->label_active->setText(tr("The switch times will be worked out when the theme is first applied."));
        return;
    }
    ui->label_active->setText(tr("Dark mode will be active between %1 and %2.")
                                  .arg(ui->inp_time_dark->time().toString(TimeFormat),
                                       ui->inp_time_light->time().toString(TimeFormat)));
}

void MainWindow::closeEvent(QCloseEvent *event)
{
    if (m_dirty) {
        const auto answer = QMessageBox::question(
            this, tr("Unsaved changes"), tr("The settings have been modified. Do you want to save them?"),
            QMessageBox::Save | QMessageBox::Discard | QMessageBox::Cancel, QMessageBox::Save);
        if (answer == QMessageBox::Cancel || (answer == QMessageBox::Save && !saveAndApply())) {
            event->ignore();
            return;
        }
        // The window may be shown again from the tray: it must not still hold what was discarded.
        if (answer == QMessageBox::Discard)
            load();
    }
    event->accept();
}
