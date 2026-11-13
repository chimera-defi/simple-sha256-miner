# v2: resumable, library-first Rust miner

Finds a nonce so that the encoded digest of `<input>:<nonce>` starts with your
prefix. A stop or crash never loses finished work and never counts work that was
not done.

## Use

```sh
cargo build --release
vanity-miner --prefix PREFIX [--input TEXT] [--algorithm sha256|blake3]
             [--encoding base64|hex] [--threads N] [--state DIR]
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

## Speed

1 thread, `nice 19`, 5 s runs, sha256, same machine, with another miner busy on
other cores. Measured by Claude.

| Hasher | MH/s (3 runs) |
| --- | --- |
| earlier hand-written AVX2 (v2 at `0002a38`) | 17.7, 17.5, 17.5 |
| `tape-sha256` (now) | 14.4, 14.5, 14.5 |

`tape-sha256` is about **18% slower** here. We took it for 100 fewer lines and
no `unsafe`. The old code is in git history if the speed matters more. 6-thread
runs were skipped to leave the live search alone; the earlier ~72.7 M/s on 6
threads came from ChatGPT's run logs and is for the old hasher. BLAKE3 is
unbenchmarked and is not lane-batched.

## Limits

Prefix is 1-10 base64 or 1-16 hex characters, matched at the start of the
encoded digest. Output is the 32-byte digest only.
