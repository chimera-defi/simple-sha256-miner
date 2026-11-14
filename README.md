# Simple SHA-256 Miner

Small CPU search for a vanity hash. It tries nonces until the **Base64 form of
`SHA-256(<phrase>:<nonce>)` starts with a chosen prefix**. It finds a real
matching hash; it never edits the digest. This is not Bitcoin mining.

**Use v2.** It was substantially faster than v1 in the reported measurements. It resumes from durably recorded completed ranges after a stop or crash; unrecorded work may need to be repeated.

## Performance snapshot (i7-8700, noisy, pre-hill-climb)

The default `--backend lib` (patched `tape-sha256`) is the fastest backend measured.
Millions of hashes per second (MH/s), 6 threads unless noted:

| Measurement | stock `tape-sha256` | custom AVX2 (`--backend avx2`) | patched lib (default) |
| --- | ---: | ---: | ---: |
| Earlier means (3-4 runs, 5 s each, load 4-7) | 63 | 79 | 97 |
| Latest paired medians (3 runs, 8 s each, load 5.4-9.7) | n/a | 70.0 | **87.6** |

Observed single-run peaks for the patched lib: **105.2** MH/s at 6 threads and **109.7**
MH/s at 11 threads, each one run. These are not sustained results. No unpinned 6-thread
run reached 110; a short pilot pinned to the six physical cores did (see below).

**Pilot, pinned to the 6 physical cores** (3 randomized lib/avx2 pairs, 10 s each, one
binary, not the planned full protocol): lib 108.6, 104.6, 99.5 vs avx2 76.9, 87.9, 80.2
MH/s; lib was faster in all 3 pairs (medians 104.6 vs 80.2). One pinned 5 s warmup run
of lib read 111.7. The host was shared and busy. Raw data: [docs/bench-pilot-20261114.txt](docs/bench-pilot-20261114.txt).

**Second, separate pilot: stock tape vs patched tape** (same source and build, only the
hash call differs; pinned to the 6 physical cores; 3 pairs of 10 s): patched 109.1, 108.6,
106.9 vs stock 82.2, 71.6, 69.3 MH/s, paired ratios 1.33, 1.52, 1.54 (median 1.52). The
host got busier during the run. This is not the lib-vs-avx2 pilot above. Archive with
provenance and raw log: [docs/bench-stock-vs-patched-20261114.txt](docs/bench-stock-vs-patched-20261114.txt).

Hill-climb experiment 1 (build the constant prefix block once per call) showed no measurable
gain in one short pilot (median paired ratio 1.02, spread 0.95-1.13) and was reverted; see
[docs/bench-hillclimb1-20261114.txt](docs/bench-hillclimb1-20261114.txt).

Conditions: one shared 12-thread i7-8700 under other jobs, unreachable 10-char prefix,
fresh state per run. There is no controlled idle-host benchmark yet, and the numbers do
not transfer to other CPUs. Links: [Detailed benchmarks](v2/README.md#speed),
[ELI5 performance explainer](docs/performance-eli5.html).

| | Language | Speed (6 threads) | Resume |
| --- | --- | ---: | --- |
| `v0/` | Python, single thread | not measured | none |
| `v1/` | C++, OpenSSL | ~2.66 M hashes/s | none |
| `v2/` | Rust, library crates | ~87-105 (snapshot above) | yes, by completed range |

The v1 figure comes from one CPU's run logs, reported by ChatGPT and not re-run by
Claude. Treat it as one data point, not a portable benchmark.

## Quick start

```sh
cd v2 && cargo build --release
target/release/vanity-miner --prefix gensys --threads 6 --state ./state | tee proof.json
target/release/vanity-miner --verify < proof.json      # recomputes everything
```

Options: `--input TEXT` (hashed as `TEXT:<nonce>`), `--algorithm sha256|blake3`,
`--encoding base64|hex`, `--threads N`, `--backend lib|avx2`. Base64 prefixes are case-sensitive by default; use `--ignore-case` to accept ASCII letter case variants (the original challenge). Hex stays lowercase and does not accept `--ignore-case`.
Each extra exact-case prefix character makes the expected work 64× (base64) or 16× (hex); in Base64 ignore-case mode, alphabetic characters add a 32× factor, while digits and symbols add 64×
larger. No promise of when a match appears.

**Recommended: the default `--backend lib`** (patched `tape-sha256`). It was the fastest
backend we measured, on one host with a noisy load.
The hand-written `--backend avx2` stays as an opt-in.

## License

The project-owned v2 code is MIT — see [LICENSE](LICENSE). The operator confirms
redistribution rights for the historical v0 and v1 files; their original
authorship has not been independently established. Dependencies retain their
own licenses, including Apache-2.0 for the Git-pinned `tape-sha256` fork.

Follow-ups: [docs/WORKLOG.md](docs/WORKLOG.md).

## Who did what

1. **Chimera (operator):** set the goal, approved the work, owns the repo.
2. **Claude (Wren):** engineering, tests, and documentation (v2 search, journal and checkpoint, validator).
3. **ChatGPT:** coordination, benchmark runs and early docs.
4. **Codex:** no verified contribution, so none is claimed.

## Challenge matching mode

For the original case-insensitive Base64 challenge, use `--prefix gensys --ignore-case`. The default remains exact-case for compatibility. Case-insensitive mode is a distinct journal/checkpoint search identity; do not reuse an exact-case state directory. Existing proofs without an `ignore_case` field are verified as exact-case. For a six-letter alphabetic prefix, case-insensitive matching has 64 times the hit probability, without changing hash throughput.
