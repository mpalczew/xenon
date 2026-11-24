#!/usr/bin/env bash

_C_RED='\033[31m' _C_GREEN='\033[32m' _C_RESET='\033[0m'

checks_quiet() {
  local name="$1" log rc=0
  shift
  log="$(mktemp)"
  "$@" >"$log" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    echo -e "${_C_GREEN}ok${_C_RESET}: $name"
  else
    cat "$log"
    echo -e "${_C_RED}failed${_C_RESET}: $name"
  fi
  rm -f "$log"
  return $rc
}

checks_all() {
  checks_quiet fmtcheck cargo fmt --all --check || return 1
  checks_quiet dprint dprint check || return 1
  checks_quiet lint-shape scripts/check-rust-shape || return 1
  checks_quiet gtk-events cargo run -q -p xenon_eventcheck || return 1
  checks_quiet lint cargo clippy --workspace --all-targets -- \
    -D warnings -D clippy::cognitive_complexity || return 1
}
