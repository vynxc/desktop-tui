#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

shell_scripts=(scripts/*.sh tools/demo/fastfetch)

for script in "${shell_scripts[@]}"; do
    bash -n "$script"
done

if command -v shellcheck >/dev/null 2>&1; then
    shellcheck "${shell_scripts[@]}"
else
    echo "shellcheck not found; skipping shell lint." >&2
fi

python3 -m py_compile tools/*.py

if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required to validate JSON files." >&2
    exit 1
fi

for json_file in \
    applet/metadata.json \
    renderer/templates/*.json; do
    jq empty "$json_file"
done

qml_linter=""
qml_formatter=""
for candidate in \
    /usr/lib/qt6/bin/qmllint \
    /usr/lib64/qt6/bin/qmllint \
    qmllint6 \
    qmllint; do
    if [[ "$candidate" = /* && -x "$candidate" ]]; then
        qml_linter="$candidate"
        break
    fi
    if [[ "$candidate" != /* ]] && command -v "$candidate" >/dev/null 2>&1; then
        qml_linter="$(command -v "$candidate")"
        break
    fi
done

for candidate in \
    /usr/lib/qt6/bin/qmlformat \
    /usr/lib64/qt6/bin/qmlformat \
    qmlformat6 \
    qmlformat; do
    if [[ "$candidate" = /* && -x "$candidate" ]]; then
        qml_formatter="$candidate"
        break
    fi
    if [[ "$candidate" != /* ]] && command -v "$candidate" >/dev/null 2>&1; then
        qml_formatter="$(command -v "$candidate")"
        break
    fi
done

qml_files=(
    applet/contents/ui/main.qml
    applet/contents/ui/configGeneral.qml
    applet/contents/config/config.qml
)

if [[ -n "$qml_linter" ]] && command -v kpackagetool6 >/dev/null 2>&1; then
    qml_help="$("$qml_linter" --help 2>&1)"
    qml_args=()
    for category in \
        import \
        incompatible-type \
        missing-property \
        missing-type \
        unqualified \
        unresolved-type; do
        if rg --quiet -- "--${category} <level>" <<<"$qml_help"; then
            qml_args+=("--${category}" disable)
        fi
    done
    "$qml_linter" "${qml_args[@]}" "${qml_files[@]}"
elif [[ -n "$qml_formatter" ]]; then
    for qml_file in "${qml_files[@]}"; do
        "$qml_formatter" --ignore-settings "$qml_file" >/dev/null
    done
else
    echo "No QML parser found; skipping QML syntax checks." >&2
fi

if rg -n \
    '/home/[^/]+|/mnt/[A-Z]+|org\.vrice|VRICE_|Re:Zero|WRATH|ram\.glb|rem\.glb|twins\.glb' \
    --glob '!vendor/**' \
    --glob '!scripts/check.sh' \
    .; then
    echo "Found private or legacy project-specific content." >&2
    exit 1
fi

git diff --check -- . ':(exclude)vendor/qmltermwidget'
echo "All checks passed."
