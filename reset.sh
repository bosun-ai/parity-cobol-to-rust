#!/usr/bin/env bash
set -euo pipefail

project="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
database_variable="${1:?database environment variable name is required}"
database_url="${!database_variable:?database URL is required}"
state_directory="${2:?state directory is required}"

mkdir -p "${state_directory}"
rm -f -- "${state_directory}/reservations.tsv"
touch "${state_directory}/reservations.tsv"
psql "${database_url}" --no-psqlrc --set ON_ERROR_STOP=1 --file "${project}/schema.sql" >/dev/null
