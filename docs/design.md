# Native Yin-Yang for KDE — design

A rewrite of [oskarsh/Yin-Yang](https://github.com/oskarsh/Yin-Yang) 4.0.1 (MIT, Python/PySide6) with no
Python dependency. Functional reference: the original sources.

## Why
The Arch package of the original runs `python3` from the `PATH` (here: miniconda) and depends on system
Python libraries (`suntime`, `dateutil`, `PySide6`, `systemd-python`). One update or a different `PATH`
and the systemd timer fails. The goal is a program whose behaviour does not change with Arch updates.

## Decisions
- **Core in Rust** (`yinyang`): CLI, sunrise/sunset computation, systemd timer, every plugin. No
  graphical toolkit, no Python. Depends only on glibc and libgcc.
- **GUI in C++/Qt 6 Widgets** (`yinyang-gui`): public Qt API only (binary compatible across the 6.x
  series). It takes theme, colours, icons and fonts from KDE through `plasma-integration`, with no KDE
  Frameworks and no `setStyle()`.
- **The GUI never writes the configuration.** It talks to the core through the CLI (`yinyang config show`,
  `config save`, `plugins --json`, `apply`, `status`). The schema is defined in one place.
- **KDE Plasma 6 only.** On other desktops the desktop-dependent plugins report "not available",
  without errors.
- **12 plugins:** system, colors, gtk, icons, kvantum, wallpaper, vs code, onlyoffice, okular, konsole,
  custom, notification.
- **No Firefox/Zen integration.** Their default theme follows the system theme (the
  `org.freedesktop.appearance` portal), so it changes together with Plasma. No extension, native
  messaging host or manifest. It can be added later if a browser-specific theme is ever needed.
- **Coexistence with the old program.** New names (`yinyang`, `yinyang.timer`, `~/.config/yinyang.json`).
  The old configuration is imported, never modified.
- **Installation as an Arch package** built from the repository with `makepkg` (`packaging/arch`), so
  that `pacman` owns the files. No AUR, AppImage or Flatpak.

## Fixes compared with the original
- #84: VS Code's `settings.json` (JSONC with comments) is no longer overwritten: only the theme value
  changes.
- #360 / location: without a network the program neither crashes nor ever uses (0, 0); it uses the last
  known location.
- #199: wallpaper with a `#` in the file name.
- #344: `--systemd` is gone; the service runs `yinyang apply --auto`.
- The timer has `Persistent=true`: if the computer was off at the switch time, the switch happens when it
  comes back (a `OnCalendar` timer that expired during suspend fires on wake-up anyway). The service
  starts after `graphical-session.target`, and `dark_mode` is not saved if a plugin fails, so the next run
  tries again.
- In "sunset to sunrise" mode the timer is realigned on every run, even when the theme does not change
  (sunrise moves every day).
- `--dry-run` really runs nothing.

## Added after review
- Every external program gets 60 s and every D-Bus call 25 s; a hung service cannot hold the lock for
  ever.
- `apply` exits with status 75 when only the Plasma session was missing; the service restarts on that
  code alone (20 s later, at most five times in ten minutes), without a notification.
- The Konsole fallback profile names no `Command`, so Konsole starts the user's own shell.

## Out of scope (for now)
GNOME, XFCE, MATE, Cinnamon, Budgie; running yinyang itself inside a Flatpak (the configuration and
extension paths of Flatpak VS Code and OnlyOffice are still handled); migrating configurations older
than 4.0 (defaults are used); GeoClue2 (IP geolocation is used); packages for other distributions.
