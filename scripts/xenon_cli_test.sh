#!/usr/bin/env bash
# Prove scripts/release/install's CLI copy, and that `xenon open` execs the app
# with XENON_DATA_DIR (does not flip slots).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/xenon-cli.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

bin_dir="$tmp/bin"
app="$tmp/Xenon.app"
mkdir -p "$bin_dir" "$app/Contents/MacOS"
cat >"$app/Contents/MacOS/xenon" <<'EOF'
#!/bin/sh
printf 'APP'
for a in "$@"; do printf ' %s' "$a"; done
printf '\n'
EOF
chmod +x "$app/Contents/MacOS/xenon"

cp "$root/scripts/xenon" "$bin_dir/xenon"
chmod +x "$bin_dir/xenon"
ln -sfn xenon "$bin_dir/xero"

[[ -x "$bin_dir/xenon" ]]
[[ "$(readlink "$bin_dir/xero")" == xenon ]]
grep -q $'^        open)$' "$bin_dir/xenon"

out="$(
    XENON_APP="$app" XENON_DATA_DIR="$tmp/data" XENON_SLOT=A \
        "$bin_dir/xenon" open /tmp/file.rs:42 --pane sibling
)"
[[ "$out" == "APP open /tmp/file.rs:42 --pane sibling" ]]

echo "ok xenon cli ($bin_dir)"
