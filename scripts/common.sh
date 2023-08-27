#!/usr/bin/env bash
usage() { grep '^###' "$0" | sed 's/^### //g; s/^###//g'; }

log_file="${LOG_FILE:-}"
if [ ! -f "$log_file" ]; then
  set -e
  log_file="$(mktemp -t "$(basename "$0")_XXXXXX.log")"
  set +e
fi
export log_file

should_use_color() {
  test -t 1 &&                      # stdout (device 1) is a tty
    test -z "${NO_COLOR:-}" &&      # the NO_COLOR variable isn't set
    command -v tput >/dev/null 2>&1 # the `tput` command is available
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
    read -r message
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

parse_rust_target() {
  local target=$1
  if [ -z "$target" ]; then
    target="$(derive_default_target)"
    return 0
  fi
  case "$target" in
  x86_64-pc-windows-msvc) printf "x86_64-pc-windows-msvc" ;;
  x86_64-unknown-linux-gnu) printf "x86_64-unknown-linux-gnu" ;;
  x86_64-apple-darwin) printf "x86_64-apple-darwin" ;;
  aarch64-apple-darwin) printf "aarch64-apple-darwin" ;;
  *) log_fail "invalid or currently-unsupported target: $target" ;;
  esac
}
