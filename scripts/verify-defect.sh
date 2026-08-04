#!/usr/bin/env bash
set -euo pipefail

project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
defect="${1:?usage: scripts/verify-defect.sh <split-reservation|missing-event>}"

case "${defect}" in
  split-reservation)
    expected="views.postgres"
    ;;
  missing-event)
    expected="views.events"
    ;;
  *)
    echo "unknown defect: ${defect}" >&2
    exit 2
    ;;
esac

work="$(mktemp -d "${TMPDIR:-/tmp}/parity-cobol-rust.XXXXXX")"
cleanup() {
  rm -rf -- "${work}"
}
trap cleanup EXIT

git -C "${project}" archive HEAD | tar -x -C "${work}"
patch -d "${work}" -p1 <"${project}/defects/${defect}.patch"
PARITY_PLAYGROUND_RUST_TARGET="${work}/target/target" "${work}/build.sh"

set +e
output="$(cd "${work}" && parity verify 2>&1)"
status=$?
set -e
printf '%s\n' "${output}"

if [[ ${status} -eq 0 ]]; then
  echo "expected Parity to reject ${defect}" >&2
  exit 1
fi
if [[ "${output}" != *"${expected}"* ]]; then
  echo "Parity rejected the target, but not at ${expected}" >&2
  exit 1
fi
