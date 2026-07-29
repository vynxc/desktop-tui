#!/usr/bin/env bash
set -euo pipefail

libexec_dir="${DESKTOP_TUI_LIBEXEC_DIR:-$HOME/.local/libexec/desktop-tui}"
plasmoid_id="io.github.vynxc.desktoptui"
restart_plasma=true

if [[ "${1:-}" == "--no-restart" ]]; then
    restart_plasma=false
elif (($# > 0)); then
    echo "Usage: ./scripts/uninstall.sh [--no-restart]" >&2
    exit 2
fi

if command -v kpackagetool6 >/dev/null 2>&1 \
    && kpackagetool6 --type Plasma/Applet --show "$plasmoid_id" >/dev/null 2>&1; then
    kpackagetool6 --type Plasma/Applet --remove "$plasmoid_id" >/dev/null
fi

rm -f -- "$libexec_dir/desktop-tui"
for asset in fox.glb LICENSE.md; do
    rm -f -- "$libexec_dir/assets/$asset"
done
for template in model-system model-sidebar model-only system system-compact; do
    rm -f -- "$libexec_dir/templates/$template.json"
done
rmdir --ignore-fail-on-non-empty "$libexec_dir/templates" 2>/dev/null || true
rmdir --ignore-fail-on-non-empty "$libexec_dir/assets" 2>/dev/null || true
rmdir --ignore-fail-on-non-empty "$libexec_dir" 2>/dev/null || true

if "$restart_plasma" \
    && command -v systemctl >/dev/null 2>&1 \
    && systemctl --user is-active --quiet plasma-plasmashell.service; then
    systemctl --user restart plasma-plasmashell.service
fi

echo "Desktop TUI was removed. Custom models and templates were left untouched."
