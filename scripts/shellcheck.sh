#!/usr/bin/env bash
declare -a to_lint=(.envrc)
for i in $(git ls-tree --full-tree -r --name-only HEAD | grep -e '.sh$'); do
  if [ -f "$i" ]; then
    to_lint+=("$i")
  fi
done
# echo "${to_lint[@]}"
shellcheck --external-sources --source-path=SCRIPTDIR "${to_lint[@]}"
