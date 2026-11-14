#!/usr/bin/env bash
# XDET: the cross-platform determinism check (#168, analysis/plan/LAN_MULTIPLAYER.md §7).
#
# Builds core's unit tests for three targets, runs the ignored determinism
# battery (core/src/sim/xdet_tests.rs) and the pinned hash tests on each, and
# compares the hash streams:
#   linux    x86_64-unknown-linux-gnu, natively
#   windows  x86_64-pc-windows-gnu, under Wine (Proton's works)
#   aarch64  aarch64-unknown-linux-musl, under user-mode qemu (the CPU stand-in
#            for Apple Silicon)
# Prints PASS when all three print the same lines and every pin passes on every
# target, FAIL otherwise with each scenario's first differing tick.
#
# Usage, from the repository root:
#   scripts/xdet.sh setup     # once: zig, the rustup target, qemu, linker wrappers
#   scripts/xdet.sh [label]   # build, run, compare; outputs in $XDET_DIR/out/<label>
#
# Environment (defaults in brackets):
#   XDET_DIR          tools and outputs [~/.local/share/openbf-xdet]
#   XDET_CARGO        the cargo command for the native build [cargo]
#   XDET_CROSS_CARGO  the cargo command for the cross builds [cargo]; RUSTFLAGS is
#                     dropped for them (a host linker such as mold must not reach
#                     a cross link) and they build into $XDET_DIR/target
#   WINE              a wine binary [wine]; a Proton files/bin/wine gets its
#                     files/lib* on WINEDLLPATH and LD_LIBRARY_PATH
#   QEMU_AARCH64      [$XDET_DIR/qemu/usr/bin/qemu-aarch64-static]
set -euo pipefail

: "${XDET_DIR:=$HOME/.local/share/openbf-xdet}"
: "${XDET_CARGO:=cargo}"
: "${XDET_CROSS_CARGO:=cargo}"
: "${WINE:=wine}"
: "${QEMU_AARCH64:=$XDET_DIR/qemu/usr/bin/qemu-aarch64-static}"
root=$(git rev-parse --show-toplevel)

# The tests whose hash literals are pinned in core, the scalar-math referee, and the max/min/clamp and
# hashed-NaN known answers (fmath: x86's answers on every CPU).
PINS=(
  fmath::tests::glam_runs_scalar_math_on_every_cpu
  fmath::tests::max_min_and_clamp_agree_on_every_cpu
  fmath::tests::a_hashed_nan_has_one_pattern
  sim::state::tests::a_squad_command_is_part_of_the_hash_and_no_command_costs_nothing
  sim::trace::tests::original_hash_pins_without_tracing
  sim::trace::tests::quantization_and_wire_hash_have_independent_known_answers
  ai::proxy::tests::an_all_proxy_skirmish_keeps_its_hash
)

write_linker() { # name zig-target extra-case-arms
  cat >"$XDET_DIR/bin/$1" <<EOF
#!/usr/bin/env bash
# Cross linker through zig cc (written by openbf scripts/xdet.sh setup).
export ZIG_GLOBAL_CACHE_DIR=$XDET_DIR/zig-cache ZIG_LOCAL_CACHE_DIR=$XDET_DIR/zig-cache
args=()
for a in "\$@"; do
  case "\$a" in
$3
    -lgcc_s|-lgcc) ;;
    *) args+=("\$a") ;;
  esac
done
exec "$XDET_DIR/venv/bin/python" -m ziglang cc -target $2 "\${args[@]}"
EOF
  chmod +x "$XDET_DIR/bin/$1"
}

setup() {
  mkdir -p "$XDET_DIR/bin" "$XDET_DIR/zig-cache" "$XDET_DIR/rpm" "$XDET_DIR/qemu"
  rustup target add aarch64-unknown-linux-musl x86_64-pc-windows-gnu
  [ -x "$XDET_DIR/venv/bin/python" ] || /usr/bin/python3 -m venv "$XDET_DIR/venv"
  "$XDET_DIR/venv/bin/pip" install -q ziglang
  if [ ! -x "$QEMU_AARCH64" ]; then
    # Fedora; elsewhere put a static qemu-aarch64 at $QEMU_AARCH64.
    (cd "$XDET_DIR/rpm" && dnf download qemu-user-static-aarch64)
    rpm2cpio "$XDET_DIR"/rpm/qemu-user-static-aarch64-*.rpm | (cd "$XDET_DIR/qemu" && cpio -idm)
  fi
  # musl: zig brings its own crt objects and unwinder. Windows: zig's LLVM
  # libunwind stands in for libgcc_eh; zig links the mingw libraries itself.
  write_linker zigcc-aarch64-musl aarch64-linux-musl \
    '    -lgcc_eh) ;;
    */self-contained/crt*.o) ;;'
  write_linker zigcc-x86_64-windows-gnu x86_64-windows-gnu \
    '    -lgcc_eh) args+=(-lunwind) ;;
    -l:libpthread.a|-lmingwex|-lmingw32|-lmsvcrt) ;;
    -Wl,--disable-auto-image-base|-Wl,--dynamicbase|-Wl,--large-address-aware) ;;
    -Wl,--high-entropy-va|-Wl,--nxcompat) ;;'
  "$XDET_DIR/venv/bin/python" -m ziglang version
  "$QEMU_AARCH64" --version | head -n 1
}

if [ "${1:-}" = setup ]; then
  setup
  exit 0
fi

label=${1:-default}
out="$XDET_DIR/out/$label"
mkdir -p "$out"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER:-$XDET_DIR/bin/zigcc-aarch64-musl}
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=${CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER:-$XDET_DIR/bin/zigcc-x86_64-windows-gnu}

# The test executable cargo reports for core's lib tests.
test_exe() {
  python3 -I -c '
import json, sys
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") \
            and m["target"]["name"] == "core" and m["profile"]["test"]:
        print(m["executable"])
'
}

build() { # target-or-empty
  cd "$root/engine"
  if [ -z "$1" ]; then
    $XDET_CARGO test -p core --lib --no-run --message-format=json-render-diagnostics | test_exe
  else
    env -u RUSTFLAGS CARGO_TARGET_DIR="$XDET_DIR/target" \
      $XDET_CROSS_CARGO test -p core --lib --no-run --target "$1" \
      --message-format=json-render-diagnostics | test_exe
  fi
}

linux_exe=$(build "" | tail -n 1)
windows_exe=$(build x86_64-pc-windows-gnu | tail -n 1)
aarch64_exe=$(build aarch64-unknown-linux-musl | tail -n 1)

wine_run() {
  local files
  files=$(cd "$(dirname "$WINE")/.." && pwd)
  if [ -d "$files/lib/wine" ]; then
    export WINEDLLPATH="$files/lib/wine:$files/lib/vkd3d"
    export LD_LIBRARY_PATH="$files/lib/x86_64-linux-gnu:$files/lib/i386-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  fi
  mkdir -p "$XDET_DIR/wineprefix"
  WINEPREFIX="$XDET_DIR/wineprefix" WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml=" \
    "$WINE" "$@"
}

run() { # name runner... exe
  local name=$1
  shift
  "$@" --ignored --nocapture --exact sim::xdet_tests::xdet_battery 2>/dev/null |
    tr -d '\r' | grep -E '^[a-z0-9_]+ [0-9]+ [0-9a-f]{16}$' >"$out/$name.txt" || true
  if "$@" --exact "${PINS[@]}" >"$out/$name.pins" 2>&1; then
    echo "pass" >"$out/$name.pinstatus"
  else
    echo "FAIL" >"$out/$name.pinstatus"
  fi
}

run linux "$linux_exe"
run windows wine_run "$windows_exe"
run aarch64 "$QEMU_AARCH64" "$aarch64_exe"

status=PASS
for t in linux windows aarch64; do
  lines=$(wc -l <"$out/$t.txt")
  echo "$t: $lines battery lines, pins $(cat "$out/$t.pinstatus")  ($out/$t.txt)"
  [ "$lines" -gt 0 ] || status=FAIL
  [ "$(cat "$out/$t.pinstatus")" = pass ] || status=FAIL
done
for t in windows aarch64; do
  if ! cmp -s "$out/linux.txt" "$out/$t.txt"; then
    status=FAIL
    echo "linux vs $t differ; first differing tick per scenario:"
    paste -d' ' "$out/linux.txt" "$out/$t.txt" | awk '
      $1 != $4 || $2 != $5 { print "  streams out of step at", $1, $2; exit }
      { n[$1]++ }
      $3 != $6 { d[$1]++; if (!($1 in f)) f[$1] = $2 }
      END { for (k in f) printf "  %-22s first tick %s (%d of %d samples)\n", k, f[k], d[k], n[k] }' | sort
  fi
done
echo "XDET $label: $status"
[ "$status" = PASS ]
