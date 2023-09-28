#!/usr/bin/env bash
### USAGE: build_bin.sh [-h|--help] [--version=base|pro]
###                      [--profile=debug|release] [--target=TARGET]
### ARGS:
###   -h|--help: print this message and exit
###   --version: base or pro (default: base)
###   --profile: debug or release (default: debug)
###   --target: one of the target identifiers listed by `rustup target list`
###     (default: x86_64-unknown-linux-gnu)
###    --zig: whether to use `cargo zigbuild` to build the binary (default: false)


if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; else this_dir=.; fi
this_dir="$(cd "${this_dir}" && pwd)"
repo_root="$(cd "${this_dir}/.." && pwd)"

RUSTFLAGS="${RUSTFLAGS:-"-Clink-args=-Wl,--build-id=sha1"}"
# shellcheck source=./common.sh
source "${this_dir}/common.sh"

find_objcopy() {
  local objcopy
  debug_path

  if is_installed llvm-objcopy; then
    log_dbug "found llvm-objcopy @ $(command -v llvm-objcopy)"
    objcopy="llvm-objcopy"
  elif is_installed objcopy; then
    log_dbug "found objcopy @ $(command -v objcopy)"
    objcopy="objcopy"
  elif is_installed gobjcopy; then
    log_dbug "found gobjcopy @ $(command -v gobjcopy)"
    objcopy="gobjcopy"
  else
    log_fail "could not find objcopy"
  fi
  printf "%s" "$objcopy"
}

derive_cargo_cmd() {
  local profile=$1
  local variant=$2
  local target=$3
  local use_zig=$4

  local cmd="cargo"
  case "$use_zig" in
    true) cmd="$cmd zigbuild" ;;
    *) cmd="$cmd build";;
  esac
  case "$profile" in
    debug) ;;
    release) cmd="$cmd --release" ;;
    *) log_fail "invalid profile: $profile" ;;
  esac

  cmd="$cmd --bin ${variant}"
  cmd="$cmd --target ${target}"
  cmd="$cmd --all-features --timings"
  printf "%s" "$cmd"
}

debug_bin_path() {
  local repo_root=$1
  find "${repo_root}/target" -type f -name '*language_server' |
    grep -v 'fingerprint' |
    while IFS= read -r line; do log_dbug "- $line"; done
}

handle_err() {
  log_errr "ERROR: exit $? @ ${BASH_SOURCE[1]} line ${BASH_LINENO[0]}"
}

build_bin() {
  local target=$1
  local version=$2
  local profile=$3
  local use_zig=$4
  cd "$repo_root" || exit 1
  local variant="${version}_language_server"
  local build_cmd=""
  build_cmd="$(derive_cargo_cmd "$profile" "$variant" "$target" "$use_zig")"

  require_cli "cargo"
  if [ "$use_zig" = true ]; then
    require_cli "zig"
    require_cli "cargo-zigbuild"
  fi
  local objcopy
  objcopy="$(find_objcopy)"
  log_dbug "using objcopy: $objcopy"

  local target_dir
  target_dir="$(derive_rust_target_dir "$repo_root" "$target" "$profile")"
  log_dbug "expected target dir: ${target_dir}"

  local bin_path
  bin_path="$(derive_rust_bin_path "$version" "$profile" "$target" "$repo_root")"
  log_dbug "expected bin path: ${bin_path}"

  # local debug_ext
  # debug_ext="$(derive_rust_debug_file_ext "$target")"
  # local debug_file="${bin_path}.${debug_ext}"
  # log_dbug "expected debug file: ${debug_file}"

  log_dbug "running: $build_cmd"
  log_info "building ${bin_path}"
  (eval "$build_cmd" 2>&1) | log_stdin "${gray}> " "${reset}"

  # debugging where the binary actually is
  # debug_bin_path "$repo_root"

  if [ ! -f "$bin_path" ]; then
    log_errr "could not find ${bin_path}"
    target_dir="$repo_root/target/$profile"
    bin_path="$target_dir/$variant"
    if [ ! -f "$bin_path" ]; then
      log_errr "could not find ${bin_path}"
      exit 1
    fi
  fi
  du -h "$bin_path" | log_info


  # log_dbug "contents of ${target_dir}:"
  # shellcheck disable=SC2012
  # ls -l "$target_dir" | log_stdin "  - "

  # strip debug symbols from the binary if we're building a release
  case "$profile" in
    debug)
    log_info "skipping debug symbol stripping for debug build"
    ;;
    release)
      # log_dbug "debug file extension: ${debug_ext}"
      # find ./target -type f -name "*.${debug_ext}" | log_stdin "  - "
      # see https://doc.rust-lang.org/rustc/codegen-options/index.html#split-debuginfo

      log_info "stripping debug symbols from ${bin_path}"
      "$objcopy" --only-keep-debug "$bin_path" "$bin_path.debug"
      log_dbug "debug symbols are stored in ${bin_path}.debug"

      log_dbug "stripping debug symbols from $bin_path"
      log_dbug "before: $(du -h "$bin_path")"
      "$objcopy" --strip-debug --strip-unneeded "$bin_path"
      log_dbug " after: $(du -h "$bin_path")"
      log_dbug "debug symbols stripped from ${bin_path}"

      log_dbug "linking debug symbols to ${bin_path}"
      "$objcopy" --add-gnu-debuglink="$bin_path.debug" "$bin_path"
      log_dbug "debug symbols linked to ${bin_path}"

  #   # TODO: upload the debug symbols to Sentry
  #   # sentry-cli debug-file upload --wait -o "${ORG}" -p "${PROJECT}" "$bin_path.debug"
    ;;
  esac
}

main() {
  set -eu -o pipefail -o errtrace
  trap handle_err ERR
  local version="${VERSION:-base}"
  local profile="${PROFILE:-debug}"
  local target="${TARGET:-x86_64-unknown-linux-gnu}"
  while [ -n "${1:-}" ]; do
    case "$1" in
    -h | --help) usage && exit 0 ;;
    --version=*) version="${1#*=}" && shift ;;
    --profile=*) profile="${1#*=}" && shift ;;
    --target=*)  target="${1#*=}"  && shift ;;
    --zig=*)     use_zig="${1#*=}" && shift ;;
    --version) version="$2" && shift 2 ;;
    --profile) profile="$2" && shift 2 ;;
    --target)  target="$2"  && shift 2 ;;
    --zig)     use_zig=true && shift   ;;
    *) echo "unexpected argument: $1" >&2 && usage >&2 && exit 1 ;;
    esac
  done
  target="$(parse_rust_target "$target")"
  version="$(parse_version "$version")"
  profile="$(parse_profile "$profile")"

  log_dbug "log file: ${log_file}"
  build_bin "$target" "$version" "$profile" "$use_zig"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main "$@"; fi
