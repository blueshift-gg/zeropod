#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

toolchain="${MIRI_TOOLCHAIN:-nightly-2026-03-27}"
logs="${MIRI_LOG_DIR:-target/miri-results}"
mkdir -p "$logs"
export MIRIFLAGS="${MIRIFLAGS:--Zmiri-strict-provenance}"
cargo "+$toolchain" miri setup

failed=0
cargo "+$toolchain" miri nextest run --workspace --all-features --no-fail-fast -j2 "$@" \
    2>&1 | tee "$logs/tests.log" || failed=1
cargo "+$toolchain" miri test --workspace --all-features --doc \
    2>&1 | tee "$logs/doctests.log" || failed=1
exit "$failed"
