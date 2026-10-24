#!/usr/bin/env bash
# Shared check helper; source it. Copied from aipm tools/lib/checks.sh (checks_quiet).

_C_RED='\033[31m' _C_GREEN='\033[32m' _C_RESET='\033[0m'

# Runs one step with its output captured: a pass prints one line, a failure prints the full log.
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
