#!/usr/bin/env bash
# Run every GPU-bound referee: the ignored `render` tests and the golden
# check, one test at a time. `OPENBF_ADAPTER=igpu|dgpu` picks the GPU. Each
# renderer also takes its turn at the GPU in code (render::device::GpuTurn).
#
#   scripts/gpu_tests.sh            # render GPU tests + goldens --check
#   scripts/gpu_tests.sh --replays  # headless sim/replay referees (GAME overrides binary)
#   scripts/gpu_tests.sh --goldens GROUP... # selected golden groups
#   scripts/gpu_tests.sh --game FILTER # ignored game suite GPU tests
#   scripts/gpu_tests.sh --game-lib [CARGO_ARGS] # serial game lib, including headless tests
#   scripts/gpu_tests.sh --battle-launch # owned Hero Assault shell launch checks
#   scripts/gpu_tests.sh --render   # only the render crate's GPU tests
#   scripts/gpu_tests.sh --hit-feedback # issue #104 captures on fresh side bakes
#   scripts/gpu_tests.sh --hud-item-layer # issue #114 real-content weapon overlap
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/engine"
export RUST_TEST_THREADS=1

if [ "${1:-}" = "--replays" ]; then
    exec "$ROOT/scripts/sim_gates.sh"
fi
if [ "${1:-}" = "--goldens" ]; then
    shift
    exec "$ROOT/scripts/goldens.sh" --check "$@"
fi

if [ "${1:-}" = "--game" ]; then
    shift
    exec cargo test -p game --test suite "$@" -- --ignored --test-threads=1 --nocapture
fi

if [ "${1:-}" = "--battle-launch" ]; then
  exec cargo test --release -p game --bin game windowed::tests::assault:: -- --ignored --test-threads=1 --nocapture
fi

if [ "${1:-}" = "--game-lib" ]; then
  shift
  for arg in "$@"; do
    if [ "$arg" = -- ]; then
      echo 'gpu_tests: --game-lib accepts Cargo arguments only' >&2
      exit 2
    fi
  done
  exec cargo test -p game --lib "$@" -- --test-threads=1
fi

if [ "${1:-}" = "--hit-feedback" ]; then
  exec cargo test -p game --test hit_feedback -- --ignored --test-threads=1
fi

if [ "${1:-}" = "--hud-item-layer" ]; then
  exec cargo test --release -p game --lib hud_feed::tests::item_layer_tests::weapon_pills_real_content_overlap -- --ignored --test-threads=1 --nocapture
fi

if [ "${1:-}" = "--ui-sharp" ]; then
  shift
  exec cargo test --release -p game --test ui_sharp "$@" -- --ignored --test-threads=1 --nocapture
fi

cargo test -p render -- --ignored --test-threads=1
[ "${1:-}" = "--render" ] && exit 0
exec "$ROOT/scripts/goldens.sh" --check
