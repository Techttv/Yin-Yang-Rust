# Switching from the original Yin-Yang to `yinyang`

`yinyang` lives next to the old program under different names (`yinyang`, `yinyang-gui`,
`yinyang.timer`, `~/.config/yinyang.json`). The package starts nothing by itself: you decide when to
switch.

## 1. Install

```sh
cd packaging/arch && makepkg -si
```

or install the `.pkg.tar.zst` from the latest release with `sudo pacman -U`.

## 2. Try it without changing anything

```sh
yinyang plugins --json | jq 'map({key,available})'
yinyang apply --dark --force --dry-run     # prints what it would do, runs nothing
yinyang status
```

`apply --auto --dry-run` says "no location" until a real run has saved a position: a dry run does not
use the network and writes nothing.

## 3. Switch over

```sh
systemctl --user disable --now yin_yang.timer   # the old timer
yinyang timer refresh                            # the new one: writes and starts yinyang.timer
systemctl --user list-timers yinyang.timer
```

The old settings (`~/.config/yin_yang*.json`) are imported the first time, and never modified.

## Rolling back

```sh
systemctl --user disable --now yinyang.timer
systemctl --user enable --now yin_yang.timer
```

## Things to know

- **Opening the old GUI re-enables `yin_yang.timer`.** The old program regenerates and starts its own
  timer whenever it is opened or saved. If that happens, disable it again with the first command of
  step 3. The old launcher entry also remains, and may be started from a session autostart.
- The first real `yinyang apply` (or the GUI's Apply button) enables `yinyang.timer`: do it after
  disabling the old one, or two timers will switch the theme.
- Firefox and Zen have no integration any more: their default theme follows the system theme through
  the `org.freedesktop.appearance` portal.
- Uninstalling the old package (`sudo pacman -Rns yin-yang`) is up to you, whenever you are sure; it is
  not needed to use `yinyang`.
