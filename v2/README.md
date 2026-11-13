# v2: resumable Rust miner

Same search as v1. A stop or crash never loses finished work, and never counts
work that was not done.

## Use

```sh
cargo build --release
target/release/gensys-miner --state DIR --prefix PREFIX --threads N \
    [--chunk C] [--start-nonce S] [--max-secs T]
python3 -I validate.py DIR/FOUND.txt   # independent check with hashlib
cargo test --release
```

Stop with SIGINT or SIGTERM. Run again with the same flags to resume. State
belongs to one search (prefix and chunk size); do not reuse it for another.
`state/` is ignored by Git.

## How it stays safe

1. Nonces are cut into fixed chunks (default 2^18), handed out in order.
2. A chunk is recorded only after every nonce in it was hashed. A chunk cut short
   by a stop, or one holding a hit, is not recorded.
3. Records go to a checksummed, fsynced journal, plus a periodic atomic
   checkpoint. A torn journal tail is ignored. A corrupt checkpoint, or changed
   settings, refuses to start.
4. A lock file stops two miners sharing one state folder.
5. A hit is re-checked with a plain scalar hash and Base64 encoder, then written
   atomically to `FOUND.txt` (input, hex digest, Base64).
6. v1's counter is never used to infer finished ranges. The v2 run started again
   from nonce zero.

## Why it is fast

Each AVX2 step hashes eight short messages at once. It checks only the leading
digest bits first, and does the full scalar check on candidates only. No single
change was isolated as the cause of the speed-up.

## Speed

- **~72.7 M hashes/s on 6 threads** (about 12 M per thread). Source: run logs on
  one CPU without SHA-NI, reported by ChatGPT; not re-run by Claude.
- **Claude's cross-check:** 7.5 M/s on one thread and 20.8 M/s on two, over 11 s
  runs while another miner used the other cores. Noisy, so it only shows the
  same order of magnitude. Python `hashlib` did about 0.8 M/s on one thread.
- Thread count and contention move these numbers.

## Limits

SHA-256 only. The input phrase is a constant in the code. Prefix is 1 to 10
Base64 characters. Without AVX2 it falls back to slow scalar checks. Hex
matching and BLAKE3 are not implemented.

## Checks

Tests cover SHA-256 vectors, the AVX2 path against the scalar path, interval
merging, and journal and checkpoint recovery (torn tail, changed settings,
corrupt checkpoint). A kill -9 test recovered completed work and resumed.
