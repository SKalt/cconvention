#!/usr/bin/env bash
declare -a to_lint
to_lint=()
for i in $(git ls-tree --full-tree -r --name-only HEAD | grep -vi lock | grep -v run_checks.sh); do
  to_lint+=("$i")
done

to_exclude=(
  '--exclude=.*\.ingest\.sentry\.io'
  '--exclude=https://indiecc.com/~skalt/conventional-commit-language-server'
  '--exclude=https://github.com/skalt/conventional-commit-language-server'
)

lychee --exclude-mail --timeout=5 --cache "${to_lint[@]}" "${to_exclude[@]}"
