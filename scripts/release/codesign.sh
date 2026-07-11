#!/usr/bin/env bash
# Shared codesign identity helpers for xero release scripts.
# shellcheck shell=bash

XERO_LOCAL_IDENTITY="${XERO_LOCAL_IDENTITY:-xero-dev}"

_xero_codesign_list() {
    security find-identity -v -p codesigning 2>/dev/null \
        | sed -n 's/^[[:space:]]*[0-9]*)[[:space:]]*[A-F0-9]*[[:space:]]*"\(.*\)"$/\1/p'
}

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

# Resolve a stable codesign identity. Prefer Apple identities, then local
# xero-dev. Never prefer bare ad-hoc (`-`) — that changes the CDHash every
# install and drops Full Disk Access / App Management grants.
xero_codesign_identity() {
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
