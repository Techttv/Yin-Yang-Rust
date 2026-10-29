#include <QApplication>
#include <QCommandLineParser>
#include <QDir>
#include <QFileInfo>

#include "appsetup.h"
#include "coreclient.h"
#include "mainwindow.h"
#include "tray.h"

// The yinyang installed next to this program, else the one in PATH: a launcher menu does not
// always have ~/.local/bin in its PATH.
static QString defaultCore()
{
    const QString sibling = QDir(QApplication::applicationDirPath()).filePath(QStringLiteral("yinyang"));
    return QFileInfo(sibling).isExecutable() ? sibling : QStringLiteral("yinyang");
}

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("yinyang-gui"));
    QApplication::setApplicationDisplayName(QStringLiteral("Yin & Yang"));
    QApplication::setApplicationVersion(QStringLiteral(PROJECT_VERSION));
    // Matches yinyang-gui.desktop, so that Plasma links the window to its launcher and icon.
    // No setStyle(): the widgets take their look from the KDE theme (plasma-integration).
    QGuiApplication::setDesktopFileName(QStringLiteral("yinyang-gui"));
    installTranslations(&app, QLocale::system());

    QCommandLineParser parser;
    parser.addHelpOption();
    parser.addVersionOption();
    const QCommandLineOption core({QStringLiteral("core")}, QObject::tr("The yinyang program to talk to."),
                                  QObject::tr("path"), defaultCore());
    const QCommandLineOption minimized(QStringLiteral("minimized"), QObject::tr("Start in the system tray."));
    parser.addOption(core);
    parser.addOption(minimized);
    parser.process(app);

    CoreClient client(parser.value(core));
    MainWindow window(&client);

    // Without a tray, closing the window is the only way out, so it must quit.
    const bool tray = QSystemTrayIcon::isSystemTrayAvailable();
    std::unique_ptr<Tray> trayIcon;
    if (tray) {
        app.setQuitOnLastWindowClosed(false);
        trayIcon = std::make_unique<Tray>(&client, &window);
        trayIcon->show();
    }
    if (!tray || !parser.isSet(minimized))
        window.show();
    return app.exec();
}
