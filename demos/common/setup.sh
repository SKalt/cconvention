#!/usr/bin/env bash
set -euo pipefail
if [[ "${BASH_SOURCE[0]}" = */* ]]; then common_dir="${BASH_SOURCE[0]%/*}"; else common_dir=.; fi
common_dir="$(cd "$common_dir" && pwd)"
repo_root="$(cd "$common_dir/../../" && pwd)"
export repo_root
temp_dir="$(mktemp -d -t "demo.XXXX")"
export temp_dir
{
  echo "export PATH='$PATH'"
  echo "export COLORTERM=truecolor"
} >>"$temp_dir"/.envrc
# ^ to avoid .envrc removing hx from $PATH
cd "$temp_dir"
direnv allow
git init &>/dev/null
touch README.md
git add .
set +eux
export GIT_CC_LS_ENABLE_TELEMETRY="true"
export GIT_CC_LS_ENABLE_ERROR_REPORTING="true"
export RUST_BACKTRACE="1"
