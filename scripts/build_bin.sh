#!/usr/bin/env bash
### USAGE: build_bin.sh [-h|--help] [--version=base|pro] [--profile=debug|release]
###                     [--target=TARGET]
### ARGS:
###   -h|--help: print this message and exit
###   --version: base or pro (default: base)
###   --profile: debug or release (default: debug)
###   --target: one of the target identifiers listed by `rustup target list`

if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "${this_dir}" && pwd)"
repo_root="$(cd "${this_dir}/.." && pwd)"

# shellcheck source=./common.sh
source "${this_dir}/common.sh"

find_objcopy() {
  local objcopy
  if is_installed objcopy; then
    objcopy="$(command -v objcopy)"
  elif is_installed gobjcopy; then
    objcopy="$(command -v gobjcopy)"
  else
    log_fail "could not find objcopy"
  fi
  printf "%s" "$objcopy"
}

build_bin() {
  local target=$1
  local version=$2
  local profile=$3
  cd "$repo_root" || exit 1
  local cargo_args=""
  case "$profile" in
  debug) ;;
  release) cargo_args="--release" ;;
  esac

  local objcopy="$(find_objcopy)"
  log_dbug "using objcopy: $objcopy"
  local bin_path="target/${target}/${profile}/${version}_language_server"
  log_info "building ${bin_path}"
  cmd="cargo build --bin ${version}_language_server --target $target $cargo_args --all-features --timings"
  log_dbug "running: $cmd"
  (eval "$cmd" 2>&1) | while IFS= read -r line; do log_dbug "${gray}> ${line}${reset}"; done
  # strip debug symbols from the binary if we're building a release
  if [ "$profile" = "release" ]; then
    log_info "stripping debug symbols from ${bin_path}"
    log_dbug "debug symbols will be stored in ${bin_path}.debug"
    "$objcopy" --only-keep-debug "$bin_path" "$bin_path.debug"
    "$objcopy" --strip-debug --strip-unneeded "$bin_path"
    "$objcopy" --add-gnu-debuglink="$bin_path.debug" "$bin_path"
    # TODO: upload the debug symbols to Sentry
    # sentry-cli upload-dif --wait -o "${ORG}" -p "${PROJECT}" "$bin_path.debug"
  fi
}

main() {
  set -euo pipefail
  local version="${VERSION:-base}"
  local profile="${PROFILE:-debug}"
  local target="${TARGET:-x86_64-unknown-linux-gnu}"
  while [ -n "${1:-}" ]; do
    case "$1" in
    -h | --help) usage && exit 0 ;;
    --version=*) version="${1#*=}" && shift ;;
    --version) version="$2" && shift 2 ;;
    --profile=*) profile="${1#*=}" && shift ;;
    --profile) profile="$2" && shift 2 ;;
    --target=*) target="${1#*=}" && shift ;;
    --target) target="$2" && shift 2 ;;
    *) echo "unexpected argument: $1" >&2 && usage >&2 && exit 1 ;;
    esac
  done
  target="$(parse_rust_target "$target")"
  version="$(parse_version "$version")"
  profile="$(parse_profile "$profile")"

  log_dbug "log file: ${log_file}"
  build_bin "$target" "$version" "$profile"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
