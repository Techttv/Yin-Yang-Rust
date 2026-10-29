#pragma once

#include <QIcon>
#include <QLocale>

class QCoreApplication;

// The application icon: the one in the icon theme once installed, else the copy built into the binary.
QIcon appIcon();

// Installs Qt's own translations and ours for `locale`. Returns whether ours exist; without them
// the interface stays in English.
bool installTranslations(QCoreApplication *app, const QLocale &locale);
