#include "appsetup.h"

#include <QCoreApplication>
#include <QLibraryInfo>
#include <QTranslator>

QIcon appIcon()
{
    return QIcon::fromTheme(QStringLiteral("yinyang"), QIcon(QStringLiteral(":/icons/icon.svg")));
}

bool installTranslations(QCoreApplication *app, const QLocale &locale)
{
    auto *qt = new QTranslator(app);
    if (qt->load(locale, QStringLiteral("qtbase"), QStringLiteral("_"),
                 QLibraryInfo::path(QLibraryInfo::TranslationsPath)))
        app->installTranslator(qt);
    else
        delete qt;

    auto *ours = new QTranslator(app);
    if (ours->load(locale, QStringLiteral("yin_yang"), QStringLiteral("."), QStringLiteral(":/i18n"))) {
        app->installTranslator(ours);
        return true;
    }
    delete ours;
    return false;
}
