#!/usr/bin/env bash
set -eu
mkdir -p ./dist
js-yaml ./src/tmLanguage.yaml >./dist/tmLanguage.json
