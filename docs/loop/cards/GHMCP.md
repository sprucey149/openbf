---
id: GHMCP
title: Evaluate headless pyghidra-mcp (clearbluejar/pyghidra-mcp) on our ghidra-proj/ for RE sessions: persistent rename / type / comment write-back, xrefs and call graph as typed tools - and answer two enabling questions, do our ELFs carry .mdebug data for ghidra-emotionengine-reloaded's STABS analyzer, and does PCSX2 save-state import work - ending in a short report and a go / no-go
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Research rec. 7. RE is 52 % of spend (about (cost omitted) per session against $8 for an ordinary card); sessions grep a 20+ MB decompile text (`analysis/SWBFRONT.ELF-decomp.c`) and re-derive names every time because nothing they learn is written back into a Ghidra database (renames live in `analysis/re-*.md` prose). Typed Ghidra tools with persistent write-back could cut the grep cost and make names accumulate. The risk is real too: another moving part in the RE loop whose four-check discipline is measured, so this is an EVALUATION with a no-go allowed.

## Evidence
- `ghidra-proj/bf1.gpr`, `bf2.gpr` (+ `.rep`) exist; `tools/ghidra_12.1.3_PUBLIC` (JDK 21 at `tools/jdk-21.0.12.1+1`) and `tools/ghidra-ee.zip` (the Emotion Engine processor module) are installed; `scripts/ExportDecomp.java` produced the decompile text.
- **.mdebug is probably absent**: `analysis/bf1-run.out` line 55 and `bf2-run.out` line 53 say `Skipping section [.mdebug.eabi64] with invalid size 0x0`, and `.mdebug.eabi64` appears in `analysis/bf1-strings.txt:6710`. That is a section HEADER with size 0 (stripped), so the STABS analyzer likely has nothing to read - to be CONFIRMED with `readelf -S` on `extracted/bf{1,2}/*.ELF` (and `.debug_*`/`.stab*` too), not assumed.
- Peers: `chaoticgd/ghidra-emotionengine-reloaded` (the maintained EE module; has a STABS analyzer and PS2 symbol handling), `cyberkaida/reverse-engineering-assistant` (ReVa), `ida-pro-mcp` (`int_convert`) - all from the research note, unverified here.

## Do (all under the capped-scope convention, no GUI)
1. Run pyghidra-mcp headless on a COPY of `ghidra-proj/bf1.gpr` (never the original; copy to `engine/artifacts/lead_tmp/ghmcp/`), as `systemd-run --user --scope --quiet --collect -p MemoryMax=8G -p MemorySwapMax=0 -p OOMPolicy=kill -- timeout <t> ...` (CLAUDE.md; Ghidra's JVM needs `-Xmx` set under that cap; Fedora/JDK from `tools/`). Pin the version and how installed (a venv under lead_tmp, no system install). No network use by the server beyond install.
2. Exercise ten typed calls on one known function pair from a recent RE card (decompile, xrefs-to / call graph, rename, retype a struct field, add a comment, search by bytes / string) and record each: worked / failed, time, memory. Repeat the rename after restarting the server: it persists?
3. Does the server's decompile of a chosen function equal the text in `SWBFRONT.ELF-decomp.c` (same function, same output; our numbering convention `FUN_<addr>`)? If it differs, by how much - this decides whether `re_check.py fn` stays the truth.
4. `readelf -S` for the debug sections of both ELFs (answer .mdebug: present and non-empty / header only / absent) and, if non-empty, load the EE module's STABS analyzer on a copy and report how many symbols/types it recovers.
5. PCSX2 save-state import: a `.p2s` is a zip; can heap/overlay structures be brought into a Ghidra program (a memory block from the state's EE RAM image, `eeMemory.bin`)? Try ONE state (`engine/artifacts/pcsx2/ref/states/`): load RAM as an extra block, and read a soldier object (BF1 component fields in `ps2live.py`'s header) through Ghidra's listing. Report success or the exact wall.
6. Cost check: run ONE short RE question twice (with the MCP, and the current grep route) and compare turns; one data point, labelled as such.

## Acceptance
- `engine/artifacts/research/decomp_landscape_20261008/ghmcp_report.md`: for each of the six items, the result with pasted commands/output; the memory peak of the server (`systemd-run` scope stats or `/proc` peak, pasted); a GO / NO-GO for adding it to the RE loop, and if GO the exact launch line (capped scope) and which of the four RE checks it replaces (none, by default - `re_check.py` stays) and which it speeds up.
- The original `ghidra-proj/` is byte-identical before and after (sha256 of the .gpr and the .rep tree listing, pasted).
- Nothing left running (`pgrep -af 'ghidra|pyghidra'`, pasted).

## Notes
Phase `re` so it runs at max effort, but this is tooling research: no `analysis/re-*.md` page is required, the report is the deliverable. If install fails for want of network or a GUI-less Java, that IS the finding: write it and stop. Cheap write-back (item 2) is the make-or-break feature; if rename persistence fails, NO-GO regardless of the rest.
