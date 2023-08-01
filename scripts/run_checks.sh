#!/usr/bin/env bash
set -euo pipefail
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
repo_root="$(cd "$this_dir/.." && pwd)"
VERSION="${VERSION:-base}"
main() {
  cd "$repo_root"
  for test in examples/*.msg; do
    expected="${test##*/}"
    expected="${expected%.msg}.expected"
    test -f "$test"
    set +e
    "${VERSION}_language_server" check -f "$test" | tee /tmp/actual
    set -e
    if [ -f "examples/$expected" ]; then
      diff -u "examples/$expected" /tmp/actual
    else
      # accept the actual ouput
      mv /tmp/actual "examples/$expected"
    fi
  done
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
