# Simple SHA-256 Miner

A small CPU vanity miner: try changing nonces until the **Base64-encoded SHA-256 digest starts with a chosen prefix**. It searches for a real matching hash; it does not insert characters into the digest.

- **`v1/`** — original C++ proof of concept, OpenSSL SHA-256, no durable resume.
- **`v2/`** — Rust, native threads, eight-lane AVX2 SHA-256, completed-range journal and restartable checkpoints.

## Run v2

```sh
cd v2
cargo build --release
./target/release/gensys-miner --prefix gensys --threads 6 --state ./state --max-secs 6900
```

The `--prefix` flag sets the **Base64 digest prefix**, case-sensitive. Current v2 input is `gensys:<decimal nonce>`; changing the input phrase, hexadecimal matching, and BLAKE3 are **not implemented yet**. State is specific to the search settings; do not reuse it across different searches. `state/` is ignored by Git.

## Measured performance

| Implementation | CPU threads | Throughput |
| --- | ---: | ---: |
| C++ v1, OpenSSL SHA-256 | 6 | ~2.66 million hashes/s |
| Rust v2, AVX2 8-lane SHA-256 | 6 | ~72.7 million hashes/s |

These are measurements from one CPU and are not portable benchmarks. Roughly **27× faster** in this experiment. V2 batches eight independent short SHA-256 messages per AVX2 operation, reduces per-hash overhead, and checks only the leading digest bits before doing a full scalar verification on a candidate. Thread count and contention also affect results; no single optimization has been isolated as the sole cause.

**Resume:** v2 persists only fully completed nonce intervals; interrupted partial chunks are retried. The original v1 progress counter cannot prove exact nonce coverage, so the v2 experiment restarted at nonce zero.

This is a CPU vanity-search experiment, not Bitcoin mining. No guarantees on time to find a match: each additional Base64 prefix character makes the expected work 64× larger.
