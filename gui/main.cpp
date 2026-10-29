#include <QApplication>
#include <QCommandLineParser>

#include "coreclient.h"
#include "mainwindow.h"

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("yinyang-gui"));
    QApplication::setApplicationVersion(QStringLiteral(PROJECT_VERSION));
    // Matches yinyang-gui.desktop, so that Plasma links the window to its launcher and icon.
    // No setStyle(): the widgets take their look from the KDE theme (plasma-integration).
    QGuiApplication::setDesktopFileName(QStringLiteral("yinyang-gui"));

    QCommandLineParser parser;
    parser.addHelpOption();
    parser.addVersionOption();
    QCommandLineOption core({QStringLiteral("core")}, QObject::tr("The yinyang program to talk to."),
                            QObject::tr("path"), QStringLiteral("yinyang"));
    parser.addOption(core);
    parser.process(app);

    CoreClient client(parser.value(core));
    MainWindow window(&client);
    window.show();
    return app.exec();
}
