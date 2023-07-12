#!/usr/bin/env bash
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "$this_dir" && pwd)"
# shellcheck source=../common/setup.sh
. "$this_dir/../common/setup.sh"
cp -r "$repo_root/.helix" "$temp_dir"
echo 'export EDITOR=hx' >>"$temp_dir/.envrc"
(cd "$temp_dir" && direnv allow)
echo "$temp_dir"
