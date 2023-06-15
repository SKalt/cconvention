#!/usr/bin/env bash
# this script assumes that it's being called as part of `npm run`:
# -../node_modules/.bin/ must be at the front of $PATH
# - the current working directory must be ${repo_root}/editors/code
set -euo pipefail
pnpm i
TARGET="${TARGET:-debug}" # TODO: use release build
variant="basic"
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "${this_dir}" && pwd)"
repo_root="$(cd "${this_dir}/../../../.." && pwd)"

# clear the dist-dir out
rm -rf ./dist && mkdir ./dist

esbuild ./src/main.ts --bundle --outfile=./dist/main.min.common.js \
  --format=cjs --platform=node --target=node18 \
  --external:vscode \
  --minify-{whitespace,identifiers,syntax} --sourcemap \
  "$@"
du -h ./dist/main.min.common.js*

js-yaml ./src/tmLanguage.yaml >./src/tmLanguage.json

cp "$repo_root/target/$TARGET/conventional-commit-language-server-${variant}" ./dist/conventional-commit-language-server
# vsix_target=./dist/git-conventional-commit-ls.vsix
# # see https://github.com/microsoft/vscode-vsce/issues/421 for issues with vsce+pnpm
# vsce package -o $vsix_target --no-dependencies # since esbuild handles that!

# unzip -l $vsix_target
# du -h $vsix_target
