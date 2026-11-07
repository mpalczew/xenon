#!/usr/bin/env bash
# Prove that `xenon open` execs the app binary with XENON_DATA_DIR (does not
# flip slots), and that running_instances sees a live GUI named xenon-bin.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/xenon-cli.XXXXXX")"
sleeper_pid=""
cleanup() {
    if [[ -n "$sleeper_pid" ]]; then
        kill "$sleeper_pid" 2>/dev/null || true
        wait "$sleeper_pid" 2>/dev/null || true
    fi
    rm -rf "$tmp"
}
trap cleanup EXIT

bin_dir="$tmp/bin"
app="$tmp/app"
mkdir -p "$bin_dir" "$app"
cat >"$app/xenon-bin" <<'EOF'
#!/bin/sh
printf 'APP'
for a in "$@"; do printf ' %s' "$a"; done
printf '\n'
EOF
chmod +x "$app/xenon-bin"

cp "$root/scripts/xenon" "$bin_dir/xenon"
chmod +x "$bin_dir/xenon"

[[ -x "$bin_dir/xenon" ]]
grep -q $'^        open)$' "$bin_dir/xenon"

out="$(
    XENON_APP_DIR="$app" XENON_DATA_DIR="$tmp/data" XENON_SLOT=A \
        "$bin_dir/xenon" open /tmp/file.rs:42 --pane sibling
)"
[[ "$out" == "APP open /tmp/file.rs:42 --pane sibling" ]]

cat >"$tmp/sleeper.c" <<'EOF'
#include <unistd.h>
int main(void) {
    sleep(60);
    return 0;
}
EOF
cc -o "$app/xenon-bin" "$tmp/sleeper.c"

home="$tmp/home"
slot_dir="$home/.xenon-a"
mkdir -p "$slot_dir"
XENON_DATA_DIR="$slot_dir" XENON_SLOT=A \
    "$app/xenon-bin" &
sleeper_pid=$!
printf '%s\n' "$sleeper_pid" >"$slot_dir/xenon.pid"

comm=""
i=0
while ((i < 50)); do
    comm="$(ps -p "$sleeper_pid" -o comm= 2>/dev/null || true)"
    comm="${comm##*/}"
    if [[ "$comm" == "xenon-bin" ]]; then
        break
    fi
    sleep 0.1
    i=$((i + 1))
done
[[ "$comm" == "xenon-bin" ]]

status_out="$(HOME="$home" XENON_APP_DIR="$app" "$bin_dir/xenon" status)"
[[ "$status_out" == *"pid $sleeper_pid slot a"* ]]
[[ -f "$slot_dir/xenon.pid" ]]
[[ "$(tr -d '[:space:]' <"$slot_dir/xenon.pid")" == "$sleeper_pid" ]]

# Pidfile gone: pgrep -x xenon-bin must still find it.
rm -f "$slot_dir/xenon.pid"
status_out="$(HOME="$home" XENON_APP_DIR="$app" "$bin_dir/xenon" status)"
[[ "$status_out" == *"pid $sleeper_pid slot a"* ]]
[[ "$(tr -d '[:space:]' <"$slot_dir/xenon.pid")" == "$sleeper_pid" ]]

echo "ok xenon cli ($bin_dir)"
