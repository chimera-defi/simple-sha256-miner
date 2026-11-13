# v2: resumable, library-first Rust miner

Finds a nonce so that the encoded digest of `<input>:<nonce>` starts with your
prefix. A stop or crash never loses finished work and never counts work that was
not done.

## Use

```sh
cargo build --release
vanity-miner --prefix PREFIX [--input TEXT] [--algorithm sha256|blake3]
             [--encoding base64|hex] [--backend lib|avx2] [--threads N] [--state DIR]
             [--chunk C] [--start-nonce S] [--max-secs T]
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
| `tape-sha256` | batch SHA-256, AVX2/AVX-512 chosen at run time | Apache-2.0 |
| `sha2` | reference SHA-256 for hits and `--verify` | MIT/Apache-2.0 |
| `blake3` | optional BLAKE3 | CC0/Apache-2.0 |
| `clap`, `base64`, `hex`, `serde_json` | CLI, encodings, proofs | MIT/Apache-2.0 |

The repo itself has no license file yet.

## Backends

`--backend lib|avx2` picks the SHA-256 engine. The default is `lib`.

- `lib`: `tape-sha256`, with a running decimal counter and reused buffers. It
  picks AVX2 or AVX-512 by itself and works on any CPU.
- `avx2`: the hand-written 8-lane AVX2 kernel from `0002a38`, in `src/avx2.rs`
  (the only `unsafe` in the crate). x86-64 with AVX2 only, sha256 only, and the
  input must be short (`len(input) + 21 < 56`). Anything else exits with an
  error. It never falls back silently.

Pick `avx2` only if it is faster on your CPU. Measure it. On the test machine it
was (6 threads, 10 s runs, 3 alternating runs each, load 3-6 from other work):

| Backend | Threads | MH/s median | Min-max |
| --- | --- | --- | --- |
| lib | 6 | 69.9 | 69.2-74.1 |
| avx2 | 6 | 82.5 | 82.3-82.6 |
| lib | 11 | 80.0 | one run |
| avx2 | 11 | 102.4 | one run |

Both backends hash the same messages and give the same digests, and every hit is
re-checked with `sha2`. The backend is not part of the state key, so a
checkpoint made with one backend resumes with the other.

## Speed

Measured by Claude with the miner itself: unreachable 10-char prefix, 10 s,
sha256, other jobs running on the same 12-core machine (load 3-6, so numbers
move). The rate is printed as `rate_MH/s` on the last line.

| Hasher | Threads | MH/s |
| --- | --- | --- |
| `tape-sha256` + running counter (default) | 6 | 69.9 median (69.2-74.1) |
| hand-written AVX2 (`--backend avx2`) | 6 | 82.5 median (82.3-82.6) |

The earlier 1-thread figures (17.7 for the old AVX2 code, 14.4 for plain
`tape-sha256`) were taken before the running counter and are not comparable to
the table. BLAKE3 is unbenchmarked and is not lane-batched.

## Limits

Prefix is 1-10 base64 or 1-16 hex characters, matched at the start of the
encoded digest. Output is the 32-byte digest only.
