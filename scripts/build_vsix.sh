#!/usr/bin/env bash
### USAGE: build_vsix.sh [-h|--help] [--version=base|pro]
###                      [--profile=debug|release] [--target=TARGET]
### ARGS:
###   -h|--help: print this message and exit
###   --version: base or pro (default: base)
###   --profile: debug or release (default: debug)
###   --target: one of the target identifiers listed by `rustup target list` OR
###             `vsce publish --help`
### ENV VARS:
###   PROFILE: see --profile (default: debug)
###   VERSION: see --version (default: base)
###   TARGET: see --target (default: x86_64-unknown-linux-gnu)

# lookaround
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "${this_dir}" && pwd)"
repo_root="$(cd "${this_dir}/.." && pwd)"

# shellcheck source=./common.sh
source "${this_dir}/common.sh"

parse_vsce_target() {
  local target=$1
  case "$target" in
  "") derive_default_target && return 0 ;;
  x86_64-pc-windows-msvc | win32-x64) printf "win32-x64" ;;
  x86_64-unknown-linux-gnu | linux-x64) printf "linux-x64" ;;
  x86_64-apple-darwin | darwin-x64) printf "darwin-x64" ;;
  aarch64-apple-darwin | darwin-arm64) printf "darwin-arm64" ;;

  # TODO: support other common targets:
  # alpine-arm64) ;;
  # alpine-x64) ;;
  # linux-arm64) ;;
  # linux-armhf) ;;
  # web) ;;
  # win32-ia32) ;;
  # win32-arm64) ;;
  *) {
    echo "invalid or currently-unsupported target: $target"
    usage
  } >&2 && exit 1 ;;
  esac
}

build_vsix() {
  local version=$1
  local profile=$2
  local target=$3
  local repo_root=$4
  local variant="${version}_language_server"
  local working_dir="${repo_root}/editors/code/$version"
  local dist_dir="$working_dir/dist"
  local vsix_path="$dist_dir/cconvention.vsix"
  local marked_path="$dist_dir/cconvention.${target}.vsix"
  cd "$working_dir" || exit 1
  # log_dbug "copying
  local original_bin_path="$repo_root/target/$profile/$variant"
  log_dbug "copying orignial bin $original_bin_path -> $dist_dir/cconvention"
  cp "$original_bin_path" "$dist_dir/cconvention"
  rm -f "$vsix_path" # just in case
  # see https://github.com/microsoft/vscode-vsce/issues/421 for issues with vsce+pnpm
  log_info "building $vsix_path"

  set +o pipefail
  # ^ yes emits error code 141 when vsce exits and breaks the pipe
  yes | vsce package -o "$vsix_path" --no-dependencies --target "$target"
  # --no-dependencies since esbuild bundles all external packages
  set -o pipefail
  cp "$vsix_path" "$marked_path"
  unzip -l "$marked_path"
  du -h "$marked_path"
}

main() {
  set -euo pipefail
  local target=${TARGET:-x86_64-unknown-linux-gnu}
  local profile="${PROFILE:-debug}"
  local version="${VERSION:-base}"

  while [ -n "${1:-}" ]; do
    case "$1" in
    -h | --help) usage && exit 0 ;;
    --profile=*) profile="${1#*=}" && shift ;;
    --profile) profile=$2 && shift 2 ;;
    --target=*) target="${1#*=}" && shift ;;
    --target) target=$2 && shift 2 ;;
    --version=*) version="${1#*=}" && shift ;;
    --version) version=$2 && shift 2 ;;
    *) echo "unexpected argument: $1" >&2 && usage >&2 && exit 1 ;;
    esac
  done

  version="$(parse_version "$version")"
  target="$(parse_vsce_target "$target")"
  profile="$(parse_profile "$profile")"

  log_dbug "profile: $profile"
  log_dbug "version: $version"
  log_dbug "target: $target"
  log_dbug "repo_root: $repo_root"

  build_vsix "$version" "$profile" "$target" "$repo_root"

}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
