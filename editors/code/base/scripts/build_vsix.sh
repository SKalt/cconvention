#!/usr/bin/env bash
### USAGE: [TARGET=debug|release] [VERSION=base|pro] ./scripts/build_vsix.sh
usage() { grep '^###' "$0" | sed 's/^### //g; s/^###//g'; }
TARGET="${TARGET:-debug}" # TODO: use release build
VERSION="${VERSION:-base}"

case "$TARGET" in
debug | release) ;;
*)
  {
    echo "TARGET must be debug or release"
    usage
  } >&2
  exit 1
  ;;
esac
case "$VERSION" in
base | pro) ;;
*)
  {
    echo "VERSION must be base or pro"
    usage
  } >&2
  exit 1
  ;;
esac

variant="${VERSION}_language_server"
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "${this_dir}" && pwd)"
repo_root="$(cd "${this_dir}/../../../.." && pwd)"

cp "$repo_root/target/$TARGET/${variant}" ./dist/conventional-commit-language-server
vsix_target=./dist/conventional-commit-language-server.vsix
rm -f "$vsix_target" # just in case
# see https://github.com/microsoft/vscode-vsce/issues/421 for issues with vsce+pnpm
yes | vsce package -o "$vsix_target" --no-dependencies # since esbuild handles that!

unzip -l $vsix_target
du -h $vsix_target
