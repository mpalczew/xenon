# xero project commands. Sourced by the global `project` launcher.
# shellcheck shell=bash

XERO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Stable signing identity name for the local self-signed fallback cert.
XERO_LOCAL_IDENTITY="${XERO_LOCAL_IDENTITY:-xero-dev}"

project_help() {
    cat <<'EOF'
xero commands:
  build     cargo build --release
  bundle    assemble target/release/xero.app (stable-signed)
  fmt       cargo fmt --all
  fmtcheck  cargo fmt --all --check
  lint      cargo clippy --workspace --all-targets -- -D warnings
  lint_shape
            enforce Rust file/function shape limits
  health    fmtcheck, lint, and test
  install   bundle and copy xero.app into ~/Applications
            (user Applications; avoids macOS App Management prompts that
            fire when agents reinstall into /Applications from inside xero)
  sign_setup
            ensure a stable codesign identity (print which one install uses)
  install_hooks
            install pre-commit hooks for this checkout
  run       cargo run (debug); pass a file path to open the editor
  test      cargo test

Signing (TCC / Full Disk Access stick across reinstalls when the identity is
stable — ad-hoc `-` does not):
  XERO_CODESIGN_IDENTITY   force an identity (codesign -s "…")
  else Developer ID Application → Apple Development → local xero-dev cert
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

# Print matching codesign identities (one per line: full quoted name).
_xero_codesign_list() {
    security find-identity -v -p codesigning 2>/dev/null \
        | sed -n 's/^[[:space:]]*[0-9]*)[[:space:]]*[A-F0-9]*[[:space:]]*"\(.*\)"$/\1/p'
}

# Create a long-lived self-signed "xero-dev" Code Signing identity in the
# login keychain when no Apple identity is available. Same CN every time so
# TCC grants (Full Disk Access, etc.) survive reinstalls.
_xero_ensure_local_identity() {
    if _xero_codesign_list | grep -qx "$XERO_LOCAL_IDENTITY"; then
        return 0
    fi
    echo "creating local codesign identity \"$XERO_LOCAL_IDENTITY\"…" >&2
    local tmp
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/xero-codesign.XXXXXX")"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" RETURN

    cat >"$tmp/openssl.cnf" <<EOF
[req]
distinguished_name = req_dn
x509_extensions = codesign_ext
prompt = no
[req_dn]
CN = ${XERO_LOCAL_IDENTITY}
[codesign_ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
EOF
    openssl req -new -x509 -days 8250 -nodes \
        -config "$tmp/openssl.cnf" \
        -keyout "$tmp/key.pem" \
        -out "$tmp/cert.pem" \
        >/dev/null 2>&1 || {
        echo "openssl failed creating $XERO_LOCAL_IDENTITY cert" >&2
        return 1
    }
    # Empty export password; partition-list below lets codesign use the key
    # non-interactively from agents / project install.
    openssl pkcs12 -export \
        -inkey "$tmp/key.pem" \
        -in "$tmp/cert.pem" \
        -out "$tmp/identity.p12" \
        -passout pass: \
        -name "$XERO_LOCAL_IDENTITY" \
        >/dev/null 2>&1 || {
        echo "openssl pkcs12 export failed" >&2
        return 1
    }
    security import "$tmp/identity.p12" \
        -k "$HOME/Library/Keychains/login.keychain-db" \
        -P "" \
        -T /usr/bin/codesign \
        -T /usr/bin/security \
        >/dev/null 2>&1 || {
        echo "security import failed for $XERO_LOCAL_IDENTITY" >&2
        return 1
    }
    # Allow codesign to use the private key without a keychain UI prompt.
    security set-key-partition-list \
        -S apple-tool:,apple:,codesign: \
        -s -k "" \
        "$HOME/Library/Keychains/login.keychain-db" \
        >/dev/null 2>&1 || true

    if ! _xero_codesign_list | grep -qx "$XERO_LOCAL_IDENTITY"; then
        echo "identity $XERO_LOCAL_IDENTITY not visible after import" >&2
        return 1
    fi
    echo "created codesign identity \"$XERO_LOCAL_IDENTITY\"" >&2
}

# Resolve a stable codesign identity. Prefer Apple identities (stable Team ID
# for TCC), then local xero-dev. Never prefer bare ad-hoc (`-`) — that changes
# the CDHash every install and drops Full Disk Access / App Management grants.
project_codesign_identity() {
    if [[ -n "${XERO_CODESIGN_IDENTITY:-}" ]]; then
        printf '%s\n' "$XERO_CODESIGN_IDENTITY"
        return 0
    fi
    local id
    id="$(_xero_codesign_list | grep '^Developer ID Application:' | head -1 || true)"
    if [[ -n "$id" ]]; then
        printf '%s\n' "$id"
        return 0
    fi
    id="$(_xero_codesign_list | grep '^Apple Development:' | head -1 || true)"
    if [[ -n "$id" ]]; then
        printf '%s\n' "$id"
        return 0
    fi
    _xero_ensure_local_identity || return 1
    printf '%s\n' "$XERO_LOCAL_IDENTITY"
}

project_sign_setup() {
    local id
    id="$(project_codesign_identity)" || return 1
    echo "codesign identity: $id"
    security find-identity -v -p codesigning 2>/dev/null | grep -F "$id" || true
}

project_bundle() {
    project_build || return 1
    local app="$XERO_ROOT/target/release/xero.app"
    local identity
    identity="$(project_codesign_identity)" || return 1
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$XERO_ROOT/macos/Info.plist" "$app/Contents/Info.plist"
    cp "$XERO_ROOT/macos/xero.icns" "$app/Contents/Resources/xero.icns"
    cp "$XERO_ROOT/target/release/xero" "$app/Contents/MacOS/xero"
    # Stable identity so TCC grants (Full Disk Access, etc.) survive reinstall.
    # --deep signs nested content; --force replaces cargo's ad-hoc linker stamp.
    codesign --force --deep --sign "$identity" "$app" || {
        echo "codesign failed with identity: $identity" >&2
        return 1
    }
    echo "bundled $app (signed: $identity)"
}

# Install under the user Applications folder, not /Applications.
# Writing to /Applications from a shell hosted by xero.app triggers macOS
# "App Management" TCC ("xero was prevented from modifying apps") every time
# an agent runs `project install`. ~/Applications is user-owned and does not.
project_install() {
    project_bundle || return 1
    local dest_dir="${XERO_INSTALL_DIR:-$HOME/Applications}"
    local dest="$dest_dir/xero.app"
    mkdir -p "$dest_dir"
    rm -rf "$dest"
    cp -R "$XERO_ROOT/target/release/xero.app" "$dest"
    # Re-sign after copy so the installed bundle's signature is intact (cp is fine
    # for same-volume but re-sign is cheap insurance for quarantine/xattrs).
    local identity
    identity="$(project_codesign_identity)" || return 1
    codesign --force --deep --sign "$identity" "$dest" || {
        echo "codesign of installed app failed: $dest" >&2
        return 1
    }
    echo "installed $dest (signed: $identity)"
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
