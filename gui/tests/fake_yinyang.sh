#!/bin/sh
# Stands in for `yinyang` in the GUI tests. $FAKE_DIR holds what it answers (plugins.json,
# config.json, defaults.json, status.json) and what it was asked (calls.log, saved.json).
echo "$*" >> "$FAKE_DIR/calls.log"
case "$1 $2" in
    "plugins --json") cat "$FAKE_DIR/plugins.json" ;;
    "config show")
        if [ "$3" = "--defaults" ]; then cat "$FAKE_DIR/defaults.json"; else cat "$FAKE_DIR/config.json"; fi ;;
    "config save")
        cat > "$FAKE_DIR/saved.json"
        if [ -n "$FAKE_SAVE_EXIT" ]; then echo "invalid config: boom" >&2; exit "$FAKE_SAVE_EXIT"; fi
        ;;
    "status "*) cat "$FAKE_DIR/status.json" ;;
    "apply "* | "toggle "*)
        [ -n "$FAKE_SLEEP" ] && sleep "$FAKE_SLEEP"
        if [ -n "$FAKE_APPLY_EXIT" ]; then echo "system: FAILED: no way" >&2; exit "$FAKE_APPLY_EXIT"; fi
        ;;
esac
