#pragma once

#include <QObject>
#include <QSystemTrayIcon>
#include <memory>

class CoreClient;
class MainWindow;
class QMenu;

// The tray icon: left click opens the window, middle click and "Toggle theme" flip the theme.
class Tray : public QObject
{
    Q_OBJECT

public:
    Tray(CoreClient *client, MainWindow *window);
    ~Tray() override;

    QMenu *menu() const { return m_menu.get(); }
    void show() { m_icon->show(); }

    void onActivated(QSystemTrayIcon::ActivationReason reason);

private:
    void showWindow();
    // Flips the theme without touching the mode: `yinyang toggle` would also switch to manual.
    void toggleTheme();
    void applied(bool ok, const QString &error);

    CoreClient *m_client;
    MainWindow *m_window;
    QSystemTrayIcon *m_icon;
    std::unique_ptr<QMenu> m_menu;
    bool m_toggling = false;
};
