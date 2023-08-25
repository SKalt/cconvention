#!/usr/bin/env bash
### USAGE: [PROFILE=debug|release] [VERSION=base|pro] ./scripts/build_vsix.sh [-h|--help]
### ENV VARS:
###   PROFILE: debug or release (default: debug)
###   VERSION: base or pro (default: base)
usage() { grep '^###' "$0" | sed 's/^### //g; s/^###//g'; }
PROFILE="${PROFILE:-debug}"
VERSION="${VERSION:-base}"

# lookaround
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "${this_dir}" && pwd)"
repo_root="$(cd "${this_dir}/../../../.." && pwd)"

build_vsix() {
  variant="${VERSION}_language_server"
  dist_dir="${repo_root}/editors/code/${VERSION}/dist"
  vsix_target="$dist_dir/cconvention.vsix"

  cp "$repo_root/target/$PROFILE/${variant}" "$dist_dir/cconvention"
  rm -f "$vsix_target" # just in case
  # see https://github.com/microsoft/vscode-vsce/issues/421 for issues with vsce+pnpm
  set +o pipefail
  # ^ yes emits error code 141 when vsce exits and breaks the pipe
  yes | vsce package -o "$vsix_target" --no-dependencies
  # --no-dependencies since esbuild bundles all external packages
  set -o pipefail
  unzip -l "$vsix_target"
  du -h "$vsix_target"
}

main() {
  set -euo pipefail
  while [ -n "${1:-}" ]; do
    case "$1" in
    -h | --help) usage && exit 0 ;;
    *) echo "unexpected argument: $1" >&2 && usage >&2 && exit 1 ;;
    esac
  done

  case "$PROFILE" in
  debug | release) ;;
  *)
    {
      echo "PROFILE must be debug or release"
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

  VERSION="$VERSION" PROFILE="$PROFILE" repo_root="$repo_root" \
    build_vsix

}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
