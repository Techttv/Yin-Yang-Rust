#!/usr/bin/env bash
# Builds yinyang and yinyang-gui and installs them for the current user, in ~/.local.
# Needs no sudo, and does not start or change any timer: see docs/CUTOVER.md for that.
set -euo pipefail

# A fixed PATH: a Python from conda or pyenv first in the user's PATH must not leak into the build.
export PATH="/usr/bin:/bin:$HOME/.cargo/bin"

root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/scalable/apps"

cd "$root"
cargo build --release --locked
cmake -S gui -B gui/build -DCMAKE_BUILD_TYPE=Release
cmake --build gui/build

install -Dm755 target/release/yinyang "$bin/yinyang"
install -Dm755 gui/build/yinyang-gui "$bin/yinyang-gui"
install -Dm644 gui/icon.svg "$icons/yinyang.svg"

# The menu may not have ~/.local/bin in its PATH, so the entry names the program by its full path.
install -d "$apps"
sed "s|^Exec=yinyang-gui\$|Exec=\"$bin/yinyang-gui\"|" gui/yinyang-gui.desktop > "$apps/yinyang-gui.desktop"
desktop-file-validate "$apps/yinyang-gui.desktop"

echo "Installed yinyang and yinyang-gui in $bin."
echo "No timer was changed; see docs/CUTOVER.md to switch over from the old Yin-Yang."
