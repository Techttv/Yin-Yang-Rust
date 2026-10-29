#include "tray.h"

#include <QCoreApplication>
#include <QIcon>
#include <QMenu>

#include "appsetup.h"
#include "coreclient.h"
#include "mainwindow.h"

namespace {
const char Context[] = "systray";
const char Comment[] = "Context menu action in the systray";
const QString Title = QStringLiteral("Yin & Yang");
}

Tray::Tray(CoreClient *client, MainWindow *window)
    : m_client(client)
    , m_window(window)
    , m_icon(new QSystemTrayIcon(appIcon(), this))
    , m_menu(new QMenu(Title))
{
    m_icon->setToolTip(Title);
    m_menu->addAction(QCoreApplication::translate(Context, "Open Yin Yang", Comment), this, &Tray::showWindow);
    m_menu->addAction(QCoreApplication::translate(Context, "Toggle theme", Comment), this, &Tray::toggleTheme);
    m_menu->addAction(QIcon::fromTheme(QStringLiteral("application-exit")),
                      QCoreApplication::translate(Context, "Quit", Comment), qApp, &QCoreApplication::quit);
    m_icon->setContextMenu(m_menu.get());

    connect(m_icon, &QSystemTrayIcon::activated, this, &Tray::onActivated);
    connect(m_client, &CoreClient::finished, this, &Tray::applied);
}

Tray::~Tray() = default;

void Tray::onActivated(QSystemTrayIcon::ActivationReason reason)
{
    if (reason == QSystemTrayIcon::Trigger)
        showWindow();
    else if (reason == QSystemTrayIcon::MiddleClick)
        toggleTheme();
}

void Tray::showWindow()
{
    m_window->show();
    m_window->raise();
    m_window->activateWindow();
}

void Tray::toggleTheme()
{
    const QJsonObject status = m_client->status();
    if (status.isEmpty()) {
        m_icon->showMessage(Title, tr("Cannot read the current theme: %1").arg(m_client->lastError()),
                            QSystemTrayIcon::Warning);
        return;
    }
    m_toggling = true;
    m_client->apply(status["dark_mode"].toBool() ? CoreClient::Light : CoreClient::Dark, true);
}

void Tray::applied(bool ok, const QString &error)
{
    if (!std::exchange(m_toggling, false) || ok)
        return;
    m_icon->showMessage(Title, tr("The theme could not be changed: %1").arg(error), QSystemTrayIcon::Warning);
}
