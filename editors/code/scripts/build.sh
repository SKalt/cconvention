#!/usr/bin/env bash
set -euo pipefail
# this script assumes that it's being called as part of `npm run`:
# -../node_modules/.bin/ must be at the front of $PATH
# - the current working directory must be ${repo_root}/editors/code
esbuild ./src/main.ts --bundle --outfile=./dist/main.min.common.js \
  --format=cjs --platform=node --target=node18 \
  --external:vscode \
  --minify-{whitespace,identifiers,syntax} --sourcemap \
  "$@"
du -h ./dist/main.min.common.js*

js-yaml ./src/tmLanguage.yaml >./src/tmLanguage.json

cp ../../target/debug/conventional-commit-language-server ./dist
# vsix_target=./dist/git-conventional-commit-ls.vsix
# rm -f $vsix_target
# vsce package -o $vsix_target

# unzip -l $vsix_target
# du -h $vsix_target
