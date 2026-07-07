# xero project commands. Sourced by the global `project` launcher.
# shellcheck shell=bash

XERO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

project_help() {
    cat <<'EOF'
xero commands:
  build     cargo build --release
  bundle    assemble target/release/xero.app (ad-hoc signed)
  install   bundle and copy xero.app into /Applications
  run       cargo run (debug); pass a file path to open the editor
  test      cargo test
EOF
}

project_build() {
    (cd "$XERO_ROOT" && cargo build --release)
}

project_bundle() {
    project_build || return 1
    local app="$XERO_ROOT/target/release/xero.app"
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$XERO_ROOT/macos/Info.plist" "$app/Contents/Info.plist"
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

project_run() {
    (cd "$XERO_ROOT" && cargo run -- "$@")
}

project_test() {
    (cd "$XERO_ROOT" && cargo test)
}
