# Worklog

Single canonical follow-up tracker. Any agent: update the row, do not start another list.

| Item | Status | Priority | Owner | Evidence | Next action | Refs |
| --- | --- | --- | --- | --- | --- | --- |
| Upstream `hash_short_heads` patch to tape-sha256 | OPEN, needs operator approval | normal | operator decides | patch in `v2/vendor/tape-sha256/PATCH.diff`; no upstream PR exists | ask operator; then open PR | 4965fa6; task-idem-9208dd0d7edc53fe3aeb76ad turn-12 |
| v0 provenance | identity unverified, operator-supplied | low | operator | `v0/miner.py` sha256 `d32648c5c72fbe8fbf65957b03661c4ef65094f10ba0293ee54c59b62333f854`, committed unchanged | none unless operator can name its source | bus 14262 |
| Nonce `u64::MAX` itself | OPEN, accepted limit | low | Claude | v2 covers `[0, 2^64-1)`; a half-open interval cannot name `u64::MAX`, so that single nonce is never tried | none unless the full domain is required | 4965fa6; checkpoint bus 14264 |
| Multi-lane BLAKE3 | OPEN | low | unassigned | `--algorithm blake3` hashes one nonce at a time | build only if BLAKE3 speed matters | v2/src/lib.rs |
| Public ELI5 site | PREPARATION approved, publication NOT approved | normal | operator decides | `docs/performance-eli5.html` (static, no scripts or external assets). Plan: a separate new public repo holding only that page (no code, no history from this repo), leak scan of the page, then GitHub Pages. This repo stays private | operator approval to create the public repo and enable Pages; link it from this README only after the URL is approved | bus 14266, 14269 |
| Backend choice | DONE: offer both, `lib` default, recommend `lib` | n/a | Claude | `--backend lib\|avx2` in v2, tests in `v2/tests/core.rs`, speeds in `v2/README.md`. Latest 6-thread paired medians (8 s, load 5.4-9.7, noisy): lib 87.6, custom 70.0 MH/s | none | ruling bus 14248; restored in 26e46f8 |
| Controlled backend benchmark | PILOT DONE (3 pairs x 10 s, pinned); full 10 x 30 s protocol still NOT RUN, needs a separate GO | normal | operator decides | `docs/bench-pilot-20261114.txt`; lib 108.6/104.6/99.5 vs avx2 76.9/87.9/80.2 MH/s, shared busy host | operator says GO for the full run | handoff 14300; rev11 |
| Repo licence | DONE for own code: MIT, copyright Chimera. OPEN: `v0/` and `v1/` authorship unverified, not covered | low | operator | root `LICENSE`; vendored `tape-sha256` stays Apache-2.0 | operator confirms the copyright holder name, and whether v0/v1 may be added | rev11 |
| Hill-climb | PROPOSED, nothing run; each experiment needs separate approval | normal | operator decides | plan below | approve experiment 1 | rev11 |

## Benchmark protocol (NOT RUN)

Do not run until the operator gives a separate GO. Compare `--backend lib` with `--backend avx2`:

1. 10 randomized, balanced AB/BA pairs of 30 s runs, 6 threads, unreachable 10-char prefix, fresh state dir each.
2. A 30 s warmup before each series.
3. Pin to the six physical cores (one thread per core, no hyperthread siblings).
4. Log CPU frequency, temperature and 1-min load before, during and after every run; discard or flag runs where they drift.
5. Report the median and range of the 10 paired differences, with the telemetry. No mining other than the benchmark itself.

Numbers so far are single-host, noisy and not portable: earlier means stock 63 / custom 79 / patched lib 97 MH/s (6 threads, 5 s, load 4-7); latest paired medians lib 87.6 / custom 70.0 (6 threads, 8 s, load 5.4-9.7); single-run peaks 105.2 (6 threads) and 109.7 (11 threads), not typical.

## Hill-climb plan (proposed, each step needs its own approval)

Baseline: pinned lib, about 100-108 MH/s. One variable per experiment, 5 paired 10 s runs each, kept only if the paired median gain exceeds the pair-to-pair spread.

1. Interleave two 8-lane groups in `heads_lanes` for more instruction-level parallelism (largest expected gain, most code).
2. Build the prefix block once and copy only the nonce digits per lane (small).
3. Hold the counter text in a precomputed block layout so no per-lane copy is needed (small).
4. Re-test batch size 64/128/256 and chunk size on pinned cores (cheap, already no gain unpinned).
5. Thread count 6 vs 12 pinned (no code).

Stop rule: any experiment that fails the digest-equivalence tests is dropped.

## Input formats

v0, v1 and v2 all hash `<input>:<unpadded decimal nonce>`. Checked: `v0/miner.py` uses `f"gensys:{nonce}"`; `v1/miner.cpp` line 35 uses `snprintf(msg, ..., "gensys:%llu", nonce)`; v2 uses `<input>:<nonce>` with `--input` defaulting to `gensys`. The default input is `gensys`. The Base64 prefix is case-sensitive. v1 and v2 start at nonce 0 (v2 can start elsewhere with `--start-nonce`).
