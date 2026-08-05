#!/usr/bin/env bash
set -euo pipefail

project="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
defect="${1:?usage: scripts/verify-defect.sh <split-reservation|missing-event>}"

case "${defect}" in
  split-reservation | missing-event) ;;
  *)
    echo "unknown defect: ${defect}" >&2
    exit 2
    ;;
esac

work="$(mktemp -d "${TMPDIR:-/tmp}/parity-cobol-rust.XXXXXX")"
offline_result="${work}/offline-result.json"
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
set +e
(cd "${work}" && parity verify --offline .parity/proof.json --json) >"${offline_result}"
offline_status=$?
set -e
if [[ ${offline_status} -eq 0 ]]; then
  echo "expected offline verification to reject ${defect}" >&2
  exit 1
fi
python3 "${project}/scripts/assert_defect.py" \
  "${defect}" "${offline_result}" "${project}/audit.contract.json"
