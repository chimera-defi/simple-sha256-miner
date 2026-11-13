# Simple SHA-256 Miner

Small CPU search for a vanity hash. It tries nonces until the **Base64 form of
`SHA-256(<phrase>:<nonce>)` starts with a chosen prefix**. It finds a real
matching hash; it never edits the digest. This is not Bitcoin mining.

**Use v2.** It is far faster than v1, and a stop or crash does not lose finished work.

## Performance snapshot (i7-8700, noisy, pre-hill-climb)

The default `--backend lib` (patched `tape-sha256`) is the fastest backend measured.
Millions of hashes per second (MH/s), 6 threads unless noted:

| Measurement | stock `tape-sha256` | custom AVX2 (`--backend avx2`) | patched lib (default) |
| --- | ---: | ---: | ---: |
| Earlier means (3-4 runs, 5 s each, load 4-7) | 63 | 79 | 97 |
| Latest paired medians (3 runs, 8 s each, load 5.4-9.7) | n/a | 70.0 | **87.6** |

Observed single-run peaks for the patched lib: **105.2** MH/s at 6 threads and **109.7**
MH/s at 11 threads. These are not sustained results, and no 6-thread run reached 110.

Conditions: one shared 12-thread i7-8700 under other jobs, unreachable 10-char prefix,
fresh state per run. There is no controlled idle-host benchmark yet, and the numbers do
not transfer to other CPUs. Details: [Detailed benchmarks](v2/README.md#speed).
Plain-words picture: [ELI5 performance explainer](docs/performance-eli5.html).

| | Language | Speed (6 threads) | Resume |
| --- | --- | ---: | --- |
| `v0/` | Python, single thread | not measured | none |
| `v1/` | C++, OpenSSL | ~2.66 M hashes/s | none |
| `v2/` | Rust, library crates | see [v2](v2/README.md#speed) | yes, by completed range |

Speeds come from one CPU's run logs, reported by ChatGPT and not re-run by
Claude. Treat them as one data point, not a portable benchmark. Details and a
second cross-check are in [`v2/README.md`](v2/README.md#speed).

## Quick start

```sh
cd v2 && cargo build --release
target/release/vanity-miner --prefix gensys --threads 6 --state ./state | tee proof.json
target/release/vanity-miner --verify < proof.json      # recomputes everything
```

Options: `--input TEXT` (hashed as `TEXT:<nonce>`), `--algorithm sha256|blake3`,
`--encoding base64|hex`, `--threads N`, `--backend lib|avx2`. Base64 prefixes are case-sensitive; hex is lowercase.
Each extra prefix character makes the expected work 64× (base64) or 16× (hex)
larger. No promise of when a match appears.

**Recommended: the default `--backend lib`** (patched `tape-sha256`). It was the fastest
backend we measured, on one host with a noisy load; see [v2 speed](v2/README.md#speed).
The hand-written `--backend avx2` stays as an opt-in.

## License

This repo has **no root license file**, so no licence or permission is granted for its
own code. The vendored `v2/vendor/tape-sha256` keeps its own Apache-2.0 licence
(`v2/vendor/tape-sha256/LICENSE`), which covers only that crate. The other crates are
used as dependencies under their own licences.

Follow-ups: [docs/WORKLOG.md](docs/WORKLOG.md). How it got faster: [visual explainer](docs/performance-eli5.html).

## Who did what

1. **Chimera (operator):** set the goal, approved the work, owns the repo.
2. **Claude (Wren):** engineering, tests, and documentation (v2 search, journal and checkpoint, validator).
3. **ChatGPT:** coordination, benchmark runs and early docs.
4. **Codex:** no verified contribution, so none is claimed.
