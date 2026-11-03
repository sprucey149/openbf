#!/usr/bin/env bash
# Run every GPU-bound referee: the ignored `render` tests and the golden
# check, one test at a time. `OPENBF_ADAPTER=igpu|dgpu` picks the GPU. Each
# renderer also takes its turn at the GPU in code (render::device::GpuTurn).
#
#   scripts/gpu_tests.sh            # render GPU tests + goldens --check
#   scripts/gpu_tests.sh --render   # only the render crate's GPU tests
#   scripts/gpu_tests.sh --hit-feedback # issue #104 captures on fresh side bakes
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/engine"

if [ "${1:-}" = "--hit-feedback" ]; then
  exec cargo test -p game --test hit_feedback -- --ignored --test-threads=1
fi

cargo test -p render -- --ignored --test-threads=1
[ "${1:-}" = "--render" ] && exit 0
exec "$ROOT/scripts/goldens.sh" --check
