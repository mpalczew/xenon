# xero project commands. Sourced by the global `project` launcher.
# shellcheck shell=bash

XERO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

project_help() {
    cat <<'EOF'
xero commands:
  build     cargo build --release
  bundle    assemble target/release/xero.app (ad-hoc signed)
  fmt       cargo fmt --all
  fmtcheck  cargo fmt --all --check
  lint      cargo clippy --workspace --all-targets -- -D warnings
  lint_shape
            enforce Rust file/function shape limits
  health    fmtcheck, lint, and test
  install   bundle and copy xero.app into /Applications
  install_hooks
            install pre-commit hooks for this checkout
  run       cargo run (debug); pass a file path to open the editor
  test      cargo test
EOF
}

project_build() {
    (cd "$XERO_ROOT" && cargo build --release)
}

project_fmt() {
    (cd "$XERO_ROOT" && cargo fmt --all)
}

project_fmtcheck() {
    (cd "$XERO_ROOT" && cargo fmt --all --check)
}

project_lint() {
    # cognitive_complexity (nursery) and too_many_lines (pedantic) are both off
    # by default, so each needs an explicit -D to enable and enforce it.
    (cd "$XERO_ROOT" && cargo clippy --workspace --all-targets -- -D warnings \
        -D clippy::cognitive_complexity \
        -D clippy::too_many_lines)
}

project_lint_shape() {
    (cd "$XERO_ROOT" && scripts/check-rust-shape)
}

project_health() {
    project_fmtcheck || return 1
    project_lint || return 1
    project_lint_shape || return 1
    project_test
}

project_bundle() {
    project_build || return 1
    local app="$XERO_ROOT/target/release/xero.app"
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$XERO_ROOT/macos/Info.plist" "$app/Contents/Info.plist"
    cp "$XERO_ROOT/macos/xero.icns" "$app/Contents/Resources/xero.icns"
    cp "$XERO_ROOT/target/release/xero" "$app/Contents/MacOS/xero"
    # Apple Silicon requires at least an ad-hoc signature to launch.
    codesign --force --deep --sign - "$app" >/dev/null 2>&1
    echo "bundled $app"
}

project_install() {
    project_bundle || return 1
    rm -rf /Applications/xero.app
    cp -R "$XERO_ROOT/target/release/xero.app" /Applications/xero.app
    echo "installed /Applications/xero.app"
}

project_install_hooks() {
    if ! command -v pre-commit >/dev/null 2>&1; then
        echo "pre-commit is not installed; install it with: brew install pre-commit" >&2
        return 1
    fi
    (cd "$XERO_ROOT" && pre-commit install)
}

project_run() {
    (cd "$XERO_ROOT" && cargo run -- "$@")
}

project_test() {
    (cd "$XERO_ROOT" && cargo test)
}
