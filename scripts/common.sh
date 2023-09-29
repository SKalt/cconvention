#!/usr/bin/env bash
usage() { grep '^###' "$0" | sed 's/^### //g; s/^###//g'; }

log_file="${LOG_FILE:-}"
if [ ! -f "$log_file" ]; then
  set -e
  log_file="$(mktemp -t "$(basename "$0")_XXXXXX.log")"
  set +e
fi
export log_file

is_installed() { command -v "$1" >/dev/null 2>&1; }

require_cli() {
  if ! is_installed "$1"; then
    log_errr "missing required CLI: $1"
    exit 127
  else
    log_dbug "found $1 @ $(command -v "$1")"
  fi
}

should_use_color() {
  test -t 1 &&                 # stdout (device 1) is a tty
    test -z "${NO_COLOR:-}" && # the NO_COLOR variable isn't set
    is_installed tput          # the `tput` command is available
}
if should_use_color; then
  red="$(tput setaf 1)"
  # green="$(tput setaf 2)"
  orange="$(tput setaf 3)"
  blue="$(tput setaf 4)"
  # purple="$(tput setaf 5)"
  # teal="$(tput setaf 6)"
  # white="$(tput setaf 7)"
  gray="$(tput setaf 11)"
  reset="$(tput sgr0)"
else
  red=""
  # green=""
  orange=""
  blue=""
  # purple=""
  # teal=""
  # white=""
  gray=""
  reset=""
fi

exec 3>&1 4>&2 # capture the overall program's stdout/err

now() { date +"%Y-%m-%dT%H:%M:%SZ"; }

log_msg() {
  local timestamp
  timestamp="$(now)"
  local level=$1
  local color=$2
  local message="${3:-}"
  if [ -z "$message" ]; then
    IFS= read -r message
  fi

  printf "%s%s\t%s%s\t%s%s\n" \
    "$color" "$level" "$gray" "$timestamp" "$reset" "$message" >&3
  printf "%s\t%s\t%s\n" \
    "$level" "$timestamp" "$message" >>"$log_file"
}

log_dbug() { log_msg "DBUG" "$gray" "$*"; }
log_info() { log_msg "INFO" "$blue" "$*"; }
log_warn() { log_msg "WARN" "$orange" "$*"; }
log_errr() { log_msg "ERRR" "$red" "$*"; }
log_fail() {
  log_errr "$*" >&4
  exit 1
}

parse_version() {
  local version=$1
  case "$version" in
  base | pro) printf "%s" "$version" ;;
  *) log_fail "VERSION must be base or pro" ;;
  esac
}

parse_profile() {
  local profile=$1
  case "$profile" in
  debug | release) printf "%s" "$profile" ;;
  *) log_fail "PROFILE must be debug or release" ;;
  esac
}

derive_default_target() {
  local os
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  local arch
  arch="$(uname -m | tr '[:upper:]' '[:lower:]')"
  case "$os" in
  linux)
    case "$arch" in
    x86_64 | amd64) printf "x86_64-unknown-linux-gnu" && return 0 ;;
    # TODO: support aarch64
    esac
    ;;
  darwin)
    case "$arch" in
    x86_64 | amd64) printf "x86_64-apple-darwin" && return 0 ;;
    aarch64 | arm64) printf "aarch64-apple-darwin" && return 0 ;;
    esac
    ;;
  windows*)
    case "$arch" in
    x86_64 | amd64) printf "x86_64-pc-windows-msvc" ;;
    esac
    ;;
  esac
  log_fail "unsupported os/arch: $os/$arch"
}

derive_rust_target_dir() {
  local repo_root=$1
  local target=$2  # e.g. x86_64-unknown-linux-gnu
  local profile=$3 # debug or release
  printf "%s/target/%s/%s" "$repo_root" "$target" "$profile"
}

derive_rust_bin_path() {
  local repo_root=$1
  local target=$2  # e.g. x86_64-unknown-linux-gnu
  local profile=$3 # debug or release
  local version=$4 # base or pro
  local variant="${version}_language_server"
  local target_dir
  target_dir="$(derive_rust_target_dir "$repo_root" "$target" "$profile")"
  local result="$target_dir/$variant"
  case "$target" in
  x86_64-pc-windows-msvc) result="$result.exe" ;;
  esac
  printf "%s" "$result"
}

parse_rust_target() {
  local target=$1
  if [ -z "$target" ]; then
    target="$(derive_default_target)"
    return 0
  fi
  case "$target" in
  x86_64-pc-windows-msvc)    printf "x86_64-pc-windows-msvc"     ;;
  x86_64-unknown-linux-gnu)  printf "x86_64-unknown-linux-gnu"   ;;
  aarch64-unknown-linux-gnu) printf "aarch64-unknown-linux-gnu"  ;;
  x86_64-apple-darwin)       printf "x86_64-apple-darwin"        ;;
  aarch64-apple-darwin)      printf "aarch64-apple-darwin"       ;;
  *) log_fail "invalid or currently-unsupported target: $target" ;;
  esac
}

derive_rust_debug_file_ext() {
  local target=$1
  if [ -z "$target" ]; then
    target="$(derive_default_target)"
  fi
  case "$target" in
  x86_64-pc-windows-msvc) printf "pdb" ;;
  x86_64-apple-darwin) printf "dSYM" ;;
  aarch64-apple-darwin) printf "dSYM" ;;
  x86_64-unknown-linux-gnu) printf "dwp" ;;
  aarch64-unknown-linux-gnu) printf "dwp" ;;
  *) log_fail "invalid or currently-unsupported target: $target" ;;
  esac
}

debug_path() {
  local i=0
  log_dbug "PATH:"
  echo "$PATH" | tr ':' '\n' | while read -r segment; do
    i=$((i + 1))
    log_dbug "  $i. $segment"
  done
}

log_stdin() {
  local prefix="${1:-}"
  local suffix="${2:-}"
  cat - | while IFS= read -r line; do
    log_dbug "${prefix}${line}${suffix}";
  done
}
