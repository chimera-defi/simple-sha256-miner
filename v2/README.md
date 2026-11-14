# v2: resumable, library-first Rust miner

Finds a nonce so that the encoded digest of `<input>:<nonce>` starts with your
prefix. A stop or crash never loses finished work and never counts work that was
not done.

## Use

```sh
cargo build --release
vanity-miner --prefix PREFIX [--input TEXT] [--algorithm sha256|blake3]
             [--encoding base64|hex] [--threads N] [--state DIR]
             [--backend lib|avx2] [--chunk C] [--start-nonce S] [--max-secs T]
vanity-miner --verify < proof.json
vanity-miner --verify --input TEXT --nonce N --digest D --prefix P \
             [--algorithm A] [--encoding E]
cargo test --release
```

Logs go to stderr. On a hit, **stdout is only the proof**, so it pipes straight
into the verifier:

```sh
vanity-miner --prefix gensy | vanity-miner --verify    # VALID gensys:60237861 -> gensyT8AF4…
```

Proof (also saved as `DIR/FOUND.json`; extra fields are ignored on verify):

```json
{ "input": "gensys", "nonce": 60237861, "message": "gensys:60237861",
  "algorithm": "sha256", "encoding": "base64", "prefix": "gensy",
  "digest": "gensyT8AF4HHhEQvm/ieh0R05yWqjtlAd3ONa/xPjdk=", "digest_hex": "…" }
```

`--verify` recomputes the digest with the `sha2` or `blake3` crate, not the
batch hasher, and exits 1 on any mismatch. `validate.py` re-checks sha256 proofs
with Python's `hashlib`.

Stop with SIGINT or SIGTERM; rerun with the same flags to resume. Any change to
input, algorithm, encoding, prefix or chunk size is refused against old state:
use a new `--state` dir.

## How it stays safe

1. Nonces are cut into fixed chunks, handed out in order.
2. A chunk is recorded only after every nonce in it was hashed. A chunk cut short
   or holding a hit is not recorded.
3. Records go to a checksummed, fsynced journal and a periodic atomic checkpoint.
   Both are bound to the search settings. A torn journal tail is ignored; a
   corrupt checkpoint, or a journal with no header and no checkpoint, refuses to
   start.
4. A lock file stops two miners sharing one state dir.
5. Every hit is re-checked by the reference crate before it is written.

## Libraries

| Crate | Use | License |
| --- | --- | --- |
| `tape-sha256` | batch SHA-256, AVX2/AVX-512 chosen at run time; vendored 0.2.2 with one added function (see below) | Apache-2.0 |
| `sha2` | reference SHA-256 for hits and `--verify` | MIT/Apache-2.0 |
| `blake3` | optional BLAKE3 | CC0/Apache-2.0 |
| `clap`, `base64`, `hex`, `serde_json` | CLI, encodings, proofs | MIT/Apache-2.0 |

The repo's own v2 code is MIT (root [`LICENSE`](../LICENSE)). The vendored
`vendor/tape-sha256` carries its own Apache-2.0 `LICENSE`, which covers only that crate.

## Speed

`tape-sha256` is vendored in `vendor/tape-sha256` (0.2.2, Apache-2.0) with one added
function, `hash_short_heads`: the first 32 bits of `SHA-256(prefix || body)` for
messages that fit one block. The miner needs only those bits to test a prefix, so
this skips the byte-wise digest output. The patch is in `vendor/tape-sha256/PATCH.diff`.
It takes the fast path on AVX2 CPUs without AVX-512 (that is the CPU measured here)
and falls back to the stock `hash_many_prefixed` for everything else, so results are
always the same. `unsafe` code lives in three places: the patch (AVX2 intrinsics), the
opt-in custom kernel `src/avx2.rs`, and the signal setup in `main.rs`. Every hit is
still re-checked with `sha2`.

**Recommendation:** use the default `--backend lib`. It is the fastest backend we
measured; the numbers below come from one noisy shared host and are not portable.

### Backends

`--backend lib` (default) uses the patched `tape-sha256` path above. `--backend avx2`
is an opt-in hand-written 8-lane kernel (`src/avx2.rs`, all its SIMD `unsafe`). It
refuses to start, with no silent fallback, without AVX2, with `--algorithm blake3`, or
when `len(input)+21 >= 56`. Both give identical digests, and the backend is not part
of the state key: a run can resume under the other one.

Latest paired run: 6 threads, 8 s each, unreachable 10-char prefix, alternating, fresh
state, shared host (1-min load 5.4 to 9.7 over the runs, so noisy). MH/s, 3 runs each;
the medians are the headline, the minimums show how much the load moved them:

| Backend | Median | Min-max |
| --- | ---: | --- |
| lib | 87.6 | 50.0-88.8 |
| avx2 | 70.0 | 46.8-70.0 |

Pick avx2 only if it measures faster on your CPU; on this host lib wins (median 87.6
against 70.0). A controlled run is planned but NOT RUN: see [docs/WORKLOG.md](../docs/WORKLOG.md).

Earlier measurements (means of 3-4 runs, 5 s each), measured with the miner itself:
unreachable 10-char prefix, alternating between builds, fresh state dir each, other jobs running on the same
12-core i7-8700 (load 4-7, so numbers move). Rate is `rate_MH/s` on the last line.

| Build | Threads | MH/s per run | Mean |
| --- | --- | --- | --- |
| stock `tape-sha256` | 6 | 58.0, 64.6, 67.8 | 63 |
| custom AVX2 (`--backend avx2`) | 6 | 76.6, 79.2, 82.4 | 79 |
| patched lib (`hash_short_heads`) | 6 | 105.2, 90.1, 95.4 | 97 |
| custom AVX2 (`--backend avx2`) | 11 | 96.9, 97.8, 97.1 | 97 |
| patched lib (`hash_short_heads`) | 11 | 108.8, 109.7, 107.9 | 109 |

The 6-thread 105.2 and the 11-thread 109.7 are single-run peaks, not typical results:
the 6-thread mean was 97 and no unpinned 6-thread run reached 110. All numbers are from one
i7-8700 under load and do not transfer to other CPUs.

### Pilot, pinned to physical cores (2026-11-14)

A short pilot, NOT the planned 10 x 30 s protocol: one binary, `taskset -c 0-5` (one thread
per physical core), 6 threads, unreachable 10-char prefix, fresh state per run, a 5 s
warmup, then 3 pairs of 10 s runs with randomized order (seed 1114). MH/s:

| Pair | lib | avx2 | Order |
| --- | ---: | ---: | --- |
| 1 | 108.61 | 76.94 | lib, avx2 |
| 2 | 104.57 | 87.88 | avx2, lib |
| 3 | 99.51 | 80.16 | lib, avx2 |

lib was faster in 3 of 3 pairs (medians 104.6 vs 80.2). The lib warmup run read 111.70.
Caveats: n = 3; the host was shared (1-min load 4.4 to 11, which includes the miner's own
6 threads); the CPU clock and package temperature (70-81 C) were sampled only before and
after each run, and some clock samples read about 2.5 GHz, so they are rough. Raw lines:
[`docs/bench-pilot-20261114.txt`](../docs/bench-pilot-20261114.txt). Pinning looks
helpful (higher than the unpinned 6-thread runs above) but that is not isolated by this
pilot.

### Stock tape vs patched tape pilot (2026-11-14)

A second bounded pilot, separate from the lib-vs-avx2 one above. Two binaries built from the
same tree (HEAD `85096a8`, same `Cargo.lock`, same vendored tape); only the call at
`src/lib.rs:141` differs: `hash_short_heads` (patched) or `hash_many_prefixed` plus
`from_be_bytes` (stock, the `b18e923` callsite). Pinned to cores 0-5, 6 threads, 4 s warmup per
arm, then 3 pairs of 10 s, order stock/patched, patched/stock, stock/patched. MH/s:

| Pair | stock | patched | patched / stock |
| --- | ---: | ---: | ---: |
| 1 | 82.20 | 109.12 | 1.33 |
| 2 | 71.55 | 108.61 | 1.52 |
| 3 | 69.27 | 106.90 | 1.54 |

Median ratio 1.52. Caveats: n = 3; the shared host's load rose from 2.3 to 5.7 during the run;
clock and temperature were single samples; "stock" still has the miner's running counter, so it
is not vanilla tape. Binary hashes, toolchain, per-run load, clock and temperature, and the
verbatim raw log and script: [`docs/bench-stock-vs-patched-20261114.txt`](../docs/bench-stock-vs-patched-20261114.txt).

Tried and not kept (no gain beyond the noise, 4 alternating runs each, 6 threads):
batch size 64 (92) and 256 (96) against 128 (95); `--chunk` 2^16 (92) and 2^20 (94)
against 2^18 (96). BLAKE3 is unbenchmarked and is not lane-batched.

## Nonce space

Nonces `0` to `2^64 - 2` are searched. `2^64 - 1` is never mined because a half-open
`[start, end)` interval cannot name it. The last chunk may be short. When every
nonce has been covered without a hit the miner prints `EXHAUSTED` and exits with
status 3; rerunning against the same state exits the same way at once.

## Limits

Prefix is 1-10 base64 or 1-16 hex characters, matched at the start of the
encoded digest. Output is the 32-byte digest only.

Follow-ups: [docs/WORKLOG.md](../docs/WORKLOG.md).
