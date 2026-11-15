# Worklog

Single canonical follow-up tracker. Any agent: update the row, do not start another list.

| Item | Status | Priority | Owner | Evidence | Next action | Refs |
| --- | --- | --- | --- | --- | --- | --- |
| Upstream `hash_short_heads` patch to tape-sha256 | PR OPEN, not merged, no reply yet | normal | operator decides | public fork chimera-defi/sha256 pinned in v2/Cargo.toml at fd76bb0, the head of spool-labs/sha256#4. The 15 Nov tests ran on 391432a, which differs only in comments and tests of the library (`docs/bench-pin-fd76bb0-20261115.txt`) | monitor upstream review; do not create a duplicate PR; when it merges and releases, switch to crates.io | 4965fa6; PR #4 |
| v0 provenance | identity unverified, operator-supplied | low | operator | `v0/miner.py` sha256 `d32648c5c72fbe8fbf65957b03661c4ef65094f10ba0293ee54c59b62333f854`, committed unchanged | none unless operator can name its source | bus 14262 |
| Nonce `u64::MAX` itself | OPEN, accepted limit | low | Claude | v2 covers `[0, 2^64-1)`; a half-open interval cannot name `u64::MAX`, so that single nonce is never tried | none unless the full domain is required | 4965fa6; checkpoint bus 14264 |
| Multi-lane BLAKE3 | OPEN | low | unassigned | `--algorithm blake3` hashes one nonce at a time | build only if BLAKE3 speed matters | v2/src/lib.rs |
| Public repo and site | DONE: repo public, GitHub Pages live | normal | operator | front page `docs/index.html`, two articles (`how-it-works-v3.html`, `search-complexity.html`) and copies on chimericlabs.xyz; `docs/performance-eli5.html` is now a pointer to the merged article | none | bus 14266, 14269 |
| Backend choice | DONE: offer both, `lib` default, recommend `lib` | n/a | Claude | `--backend lib\|avx2` in v2, tests in `v2/tests/core.rs`, speeds in `v2/README.md`. Latest 6-thread paired medians (8 s, load 5.4-9.7, noisy): lib 87.6, custom 70.0 MH/s | none | ruling bus 14248; restored in 26e46f8 |
| Controlled backend benchmark | PILOT DONE (3 pairs x 10 s, pinned); full 10 x 30 s protocol still NOT RUN, needs a separate GO | normal | operator decides | `docs/bench-pilot-20261114.txt`; lib 108.6/104.6/99.5 vs avx2 76.9/87.9/80.2 MH/s, shared busy host | operator says GO for the full run | handoff 14300; rev11 |
| Repo licence | v2 MIT operator-confirmed; v0/v1 redistribution rights operator-confirmed, original authorship independently unverified | low | operator | root `LICENSE`; `tape-sha256` Git-pinned fork remains Apache-2.0; full Git-history email disclosure operator-approved | repo is public; build and 19 tests pass on the pinned fork | checkpoint rev31 |
| Stock-vs-patched tape pilot | DONE (second pilot, separate from lib-vs-avx2): 3 pairs x 10 s pinned, median patched/stock 1.52 | normal | Claude | `docs/bench-stock-vs-patched-20261114.txt` (raw log, script, binary hashes) | none; full 10 x 30 s protocol still NOT RUN | rev13; bus 14321 |
| Hill-climb experiment 1 (prefix template in `heads_lanes`) | DONE, INCONCLUSIVE, rolled back; the one approved experiment is used up | normal | Claude | `docs/bench-hillclimb1-20261114.txt`: cand/base paired ratios 1.02, 1.13, 0.95 (median 1.02, sd 0.09, n=3), tests 15/15 | any further experiment needs a new operator approval | rev15 |
| Hill-climb | Experiment 1 ran and was rolled back (row above); steps 1, 3, 4 and 5 NOT RUN, each needs separate approval | normal | operator decides | plan below | approve the next experiment, or close | rev11 |
| C++ vs Rust pilot (v1 against v2) | DONE, 15 Nov: 2.62 against 109.1 MH/s, median ratio 41.6 (4 pairs, 10 s, pinned, on 391432a) | normal | Claude | `docs/bench-cpp-vs-rust-20261115.txt`; articles `docs/how-it-works-v3.html`, `docs/search-complexity.html` | none; full protocol NOT RUN | PR #8, #9 |
| Pin check 391432a to fd76bb0 | DONE, 15 Nov: same output, speed within noise, 19 tests pass | low | Claude | `docs/bench-pin-fd76bb0-20261115.txt` | none | this change |

## Benchmark protocol (NOT RUN)

Do not run until the operator gives a separate GO. Compare `--backend lib` with `--backend avx2`:

1. 10 randomized, balanced AB/BA pairs of 30 s runs, 6 threads, unreachable 10-char prefix, fresh state dir each.
2. A 30 s warmup before each series.
3. Pin to the six physical cores (one thread per core, no hyperthread siblings).
4. Log CPU frequency, temperature and 1-min load before, during and after every run; discard or flag runs where they drift.
5. Report the median and range of the 10 paired differences, with the telemetry. No mining other than the benchmark itself.

Numbers so far (before the 15 Nov tests; the 42 times result is in the C++ vs Rust row) are single-host, noisy and not portable: earlier means stock 63 / custom 79 / patched lib 97 MH/s (6 threads, 5 s, load 4-7); latest paired medians lib 87.6 / custom 70.0 (6 threads, 8 s, load 5.4-9.7); single-run peaks 105.2 (6 threads) and 109.7 (11 threads), not typical.

## Hill-climb plan (step 2 ran and was rolled back; the rest need their own approval)

Baseline: pinned lib, about 100-108 MH/s. One variable per experiment, 5 paired 10 s runs each, kept only if the paired median gain exceeds the pair-to-pair spread.

1. Interleave two 8-lane groups in `heads_lanes` for more instruction-level parallelism (largest expected gain, most code).
2. Build the prefix block once and copy only the nonce digits per lane (small). RAN as experiment 1: no gain, rolled back (`docs/bench-hillclimb1-20261114.txt`).
3. Hold the counter text in a precomputed block layout so no per-lane copy is needed (small).
4. Re-test batch size 64/128/256 and chunk size on pinned cores (cheap, already no gain unpinned).
5. Thread count 6 vs 12 pinned (no code).

Stop rule: any experiment that fails the digest-equivalence tests is dropped.

## Input formats

v0, v1 and v2 all hash `<input>:<unpadded decimal nonce>`. Checked: `v0/miner.py` uses `f"gensys:{nonce}"`; `v1/miner.cpp` line 35 uses `snprintf(msg, ..., "gensys:%llu", nonce)`; v2 uses `<input>:<nonce>` with `--input` defaulting to `gensys`. The default input is `gensys`. The Base64 prefix is case-sensitive in v0/v1 and by default in v2; v2 `--ignore-case` accepts ASCII case variants. v1 and v2 start at nonce 0 (v2 can start elsewhere with `--start-nonce`).

- **Ignore-case (commit `12569fc`):** v2 `--ignore-case` Base64 search mode has a masked first-word filter, verifier/proof compatibility and separate durable state identity. Historical v0/v1 remain exact-case. The repo and GitHub Pages are public.
