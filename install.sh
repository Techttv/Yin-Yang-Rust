#!/usr/bin/env bash
# Installs yinyang and yinyang-gui for the current user, in ~/.local, without pacman and without sudo.
#
#   ./install.sh               build and install (run it again to update)
#   ./install.sh --uninstall   remove what it installed
#
# It starts and changes no timer: see docs/MIGRATING.md. To have pacman own the files instead, use the
# package in packaging/arch.
set -euo pipefail

# A fixed PATH: a Python from conda or pyenv first in the user's PATH must not leak into the build.
export PATH="/usr/bin:/bin:$HOME/.cargo/bin"

root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/scalable/apps"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

case "${1:-}" in
    "") ;;
    --uninstall)
        rm -fv "$bin/yinyang" "$bin/yinyang-gui" "$apps/yinyang-gui.desktop" "$icons/yinyang.svg" \
            "$autostart/yinyang-gui.desktop"
        echo "Your settings (~/.config/yinyang.json) and the timer were left alone."
        echo "To stop the timer too: systemctl --user disable --now yinyang.timer"
        exit 0
        ;;
    *)
        echo "usage: $0 [--uninstall]" >&2
        exit 2
        ;;
esac

# Two copies, one in /usr/bin and one in ~/.local/bin, would shadow each other and drift apart.
if pacman -Qq yinyang >/dev/null 2>&1; then
    echo "yinyang is installed by pacman: remove it first (sudo pacman -R yinyang) or keep using pacman." >&2
    exit 1
fi

cd "$root"
cargo build --release --locked
cmake -S gui -B gui/build -DCMAKE_BUILD_TYPE=Release
cmake --build gui/build

install -Dm755 target/release/yinyang "$bin/yinyang"
install -Dm755 gui/build/yinyang-gui "$bin/yinyang-gui"
install -Dm644 gui/icon.svg "$icons/yinyang.svg"

# The session may not have ~/.local/bin in its PATH, so the entries name the program by its full path.
install -d "$apps" "$autostart"
sed "s|^Exec=yinyang-gui\$|Exec=\"$bin/yinyang-gui\"|" gui/yinyang-gui.desktop > "$apps/yinyang-gui.desktop"
# The tray icon starts with the session, minimized. The timer needs no GUI; this only keeps the icon.
sed "s|^Exec=yinyang-gui --minimized\$|Exec=\"$bin/yinyang-gui\" --minimized|" gui/yinyang-gui-autostart.desktop > "$autostart/yinyang-gui.desktop"
desktop-file-validate "$apps/yinyang-gui.desktop" "$autostart/yinyang-gui.desktop"

echo "Installed yinyang and yinyang-gui in $bin, and the tray icon in the session autostart."
echo "No timer was changed; see docs/MIGRATING.md to switch over from the old Yin-Yang."
