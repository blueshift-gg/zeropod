#!/usr/bin/env bash
# Every test under Miri, with strict provenance. Property tests run four
# cases: Miri is slow, and the point is each unsafe path, not the inputs.
set -euo pipefail
cd "$(dirname "$0")/.."
toolchain="${MIRI_TOOLCHAIN:-nightly-2026-03-27}"
export MIRIFLAGS="${MIRIFLAGS:--Zmiri-strict-provenance} -Zmiri-disable-isolation"
export PROPTEST_CASES="${PROPTEST_CASES:-4}"
cargo "+$toolchain" miri test --all-features "$@"
