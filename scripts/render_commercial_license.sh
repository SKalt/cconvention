#!/usr/bin/env bash
### USAGE: $0 NAME LOCATION EMAIL N_USERS PRICE
usage() { grep '^###' "$0"  | sed "s/^### //g; s/^###//g; s#\\\$0#$0#;"; }
iso_date() { date +"%Y-%m-%d"; }
set -eu

while [ -n "${1:-}" ]; do
  case "$1" in
    -h|--help) usage && exit 0;;
    -*) echo "unexpected argument: $1" >&2 && usage >&2 && exit 1;;
    *) if [ -z "${CUSTOMER_NAME:-}" ]; then
         CUSTOMER_NAME="$1"
       elif [ -z "${CUSTOMER_LOCATION:-}" ]; then
         CUSTOMER_LOCATION="$1"
       elif [ -z "${CUSTOMER_EMAIL:-}" ]; then
         CUSTOMER_EMAIL="$1"
       elif [ -z "${N_USERS:-}" ]; then
         N_USERS="$1"
       elif [ -z "${PRICE:-}" ]; then
         PRICE="$1"
       else
         echo "unexpected argument: $1" >&2 && usage >&2 && exit 1
       fi
       shift
       ;;
  esac
done

sed "
    s/DATE/$(iso_date)/1
    s/CUSTOMER_NAME/${CUSTOMER_NAME}/1
    s/CUSTOMER_LOCATION/${CUSTOMER_LOCATION}/1
    s/CUSTOMER_EMAIL/${CUSTOMER_EMAIL}/1
    s/USERS/${N_USERS}/1
    s/PRICE/${PRICE:-10}/1
  " <./LICENSES/COMMERCIAL.template.md
