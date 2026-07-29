#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
libexec_dir="${DESKTOP_TUI_LIBEXEC_DIR:-$HOME/.local/libexec/desktop-tui}"
plasmoid_id="io.github.vynxc.desktoptui"
plasmoid_dir="$data_home/plasma/plasmoids/$plasmoid_id"
restart_plasma=true

usage() {
    cat <<'EOF'
Usage: ./scripts/install.sh [--no-restart]

Build and install Desktop TUI for the current user.

Options:
  --no-restart  Do not restart Plasma Shell after installation.
  -h, --help    Show this help.
EOF
}

while (($# > 0)); do
    case "$1" in
        --no-restart)
            restart_plasma=false
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

required_commands=(cargo kpackagetool6 make)
for command_name in "${required_commands[@]}"; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "Missing required command: $command_name" >&2
        echo "See the dependency list in README.md." >&2
        exit 1
    fi
done

qmake_command="$(command -v qmake6 || command -v qmake || true)"
if [[ -z "$qmake_command" ]]; then
    echo "Missing Qt 6 qmake (expected qmake6 or qmake)." >&2
    exit 1
fi
if [[ "$("$qmake_command" -query QT_VERSION 2>/dev/null || true)" != 6.* ]]; then
    echo "$qmake_command is not a Qt 6 qmake." >&2
    exit 1
fi

jobs="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)}"
build_dir="$(mktemp -d)"
trap 'rm -rf -- "$build_dir"' EXIT

echo "Building the Rust renderer..."
cargo build \
    --manifest-path "$project_dir/Cargo.toml" \
    --package desktop-tui \
    --release \
    --locked

echo "Building the transparent terminal module..."
mkdir -p "$build_dir/qmltermwidget"
(
    cd "$build_dir/qmltermwidget"
    "$qmake_command" "$project_dir/vendor/qmltermwidget/qmltermwidget.pro"
    make --silent -j"$jobs"
)

echo "Installing Desktop TUI for the current user..."
install -Dm755 \
    "$project_dir/target/release/desktop-tui" \
    "$libexec_dir/desktop-tui"
install -d "$libexec_dir/assets" "$libexec_dir/templates"
install -m644 "$project_dir"/renderer/assets/* "$libexec_dir/assets/"
install -m644 "$project_dir"/renderer/templates/*.json "$libexec_dir/templates/"

package_dir="$build_dir/$plasmoid_id"
mkdir -p "$package_dir"
cp -a "$project_dir/applet/." "$package_dir/"
cp -a "$build_dir/qmltermwidget/QMLTermWidget" \
    "$package_dir/contents/ui/QMLTermWidget"
install -Dm644 \
    "$project_dir/applet/color-schemes/DesktopTuiTransparent.colorscheme" \
    "$package_dir/contents/ui/QMLTermWidget/color-schemes/DesktopTuiTransparent.colorscheme"

if [[ -d "$plasmoid_dir" ]]; then
    kpackagetool6 --type Plasma/Applet --upgrade "$package_dir" >/dev/null
    cp -a "$package_dir/." "$plasmoid_dir/"
else
    kpackagetool6 --type Plasma/Applet --install "$package_dir" >/dev/null
fi

if "$restart_plasma"; then
    if command -v systemctl >/dev/null 2>&1 \
        && systemctl --user is-active --quiet plasma-plasmashell.service; then
        systemctl --user restart plasma-plasmashell.service
    else
        echo "Plasma Shell was not restarted; log out and back in if the widget is not listed."
    fi
fi

echo
echo "Desktop TUI is installed."
echo "Open Plasma's widget picker and add 'Desktop TUI' to the desktop."
