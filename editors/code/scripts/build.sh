#!/usr/bin/env bash
# this script assumes that it's being called as part of `npm run`:
# -../node_modules/.bin/ must be at the front of $PATH
# - the current working directory must be ..
esbuild ./src/main.ts --bundle --outfile=./dist/main.min.common.js \
  --format=cjs --platform=node --target=node18 \
  --external:vscode \
  --minify-{whitespace,identifiers,syntax} --sourcemap \
  "$@"
# TODO: jsonify the grammar
./node_modules/.bin/js-yaml ./src/tmLanguage.yaml >./src/tmLanguage.json
