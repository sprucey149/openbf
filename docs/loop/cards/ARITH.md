---
id: ARITH
title: A tiny deterministic arithmetic helper for RE sessions - int convert (hex/dec/signed/float bits), struct-offset arithmetic, MIPS lui/addiu|ori|lw|sw pair resolver - so a session calls a tool instead of doing address arithmetic in its head
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Research rec. 7 (Lewis; ida-pro-mcp ships `int_convert` for this reason): struct-offset and address arithmetic is a known LLM failure, and a wrong address in an `analysis/re-*.md` claim is exactly the confident-false-[V] class our blind tests measured (4 false [V] in the Opus RE trial). The cost of a tool is a script; the cost of the error is a wrong card.

## Evidence
- `scripts/re_check.py` already does check 1 (`fn`), 2 (`absent`), 4 (`word`: ELF bytes at a VA with the offset formula `off = va - 0x160000 + 0x1000`), 5 (`writers`) - none of it does arithmetic on constants a session has read off a listing. Decompile text shows addresses as `DAT_0073d870`, `0x532D20`, `iVar + 0x228`, and MIPS pairs as `lui v0,0x73` / `addiu v0,v0,-0x2790` (signed low half: the classic mistake).

## Acceptance
- `scripts/re_calc_selftest.sh` (wired into `scripts/gates_selftest.py`): >= 20 cases including `lui 0x73 / addiu -0x2790` = 0x72D870 (signed borrow), a `lui 0x73`/`ori` unsigned case, a negative offset, an f32 round trip (0x3f800000 = 1.0), `va2off` both ways on a known VA from `re_check.py word`'s docstring, and one decoded instruction pair from the real BF1 ELF at the LCG's address `0x66eed0` (BF1 `DAT_0066eed0`, `analysis/re-blaster-damage.md`) pasted with its output.
- LOOP_PROMPT's RE section gets ONE line: any address, offset or lui/addiu result written into an RE doc comes from `re_calc.py` (output pasted), not from the model.
- Runs in < 0.5 s, no network, no file writes.

## Notes
Keep it a single file under ~200 lines; no MCP wrapper (research suggests a typed tool, but a CLI the RE session already calls via Bash is the smallest thing). If GHMCP lands GO, an MCP wrapper can be a follow-up.

## GitHub checkpoint recovery (2026-10-24)
