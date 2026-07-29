#!/usr/bin/env bash
set -euo pipefail

missing=0

check_command() {
    local command_name="$1"
    local purpose="$2"

    if command -v "$command_name" >/dev/null 2>&1; then
        printf 'ok      %-18s %s\n' "$command_name" "$purpose"
    else
        printf 'missing %-18s %s\n' "$command_name" "$purpose"
        missing=1
    fi
}

echo "Desktop TUI environment"
echo

check_command cargo "Rust renderer build"
check_command fastfetch "System-information templates"
check_command kpackagetool6 "Plasma applet installation"
check_command make "Native build orchestration"

qmake_command="$(command -v qmake6 || command -v qmake || true)"
if [[ -n "$qmake_command" && "$("$qmake_command" -query QT_VERSION 2>/dev/null || true)" == 6.* ]]; then
    printf 'ok      %-18s %s\n' "$(basename "$qmake_command")" "Qt 6 terminal module build"
else
    printf 'missing %-18s %s\n' "qmake6" "Qt 6 terminal module build"
    missing=1
fi

echo
if command -v plasmashell >/dev/null 2>&1; then
    plasmashell --version
else
    echo "Plasma Shell was not found."
    missing=1
fi

if [[ "$missing" -ne 0 ]]; then
    echo
    echo "Install the missing dependencies listed in README.md, then run this again."
    exit 1
fi

echo
echo "Required runtime and build tools are available."
