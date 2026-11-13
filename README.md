# Simple SHA-256 Miner

Small CPU search for a vanity hash. It tries nonces until the **Base64 form of
`SHA-256(<phrase>:<nonce>)` starts with a chosen prefix**. It finds a real
matching hash; it never edits the digest. This is not Bitcoin mining.

**Use v2.** It is about 27× faster than v1 in our one measurement, and a stop or
crash does not lose finished work.

| | Language | Speed (6 threads) | Resume |
| --- | --- | ---: | --- |
| `v1/` | C++, OpenSSL | ~2.66 M hashes/s | none |
| `v2/` | Rust, AVX2 8-lane | ~72.7 M hashes/s | yes, by completed range |

Speeds come from one CPU's run logs, reported by ChatGPT and not re-run by
Claude. Treat them as one data point, not a portable benchmark. Details and a
second cross-check are in [`v2/README.md`](v2/README.md#speed).

## Quick start

```sh
cd v2
cargo build --release
./target/release/gensys-miner --prefix gensys --threads 6 --state ./state
```

Prefix match is case-sensitive. Each extra prefix character makes the expected
work 64× larger; there is no promise of when a match appears.

## Not built yet

- A custom input phrase (v2 uses `gensys:<decimal nonce>`, fixed in code).
- Hex matching.
- BLAKE3.

## Who did what

1. **Chimera (operator):** set the goal, approved the work, owns the repo.
2. **Claude:** engineered v2 (search, journal and checkpoint, tests, validator).
3. **ChatGPT:** coordination, benchmark runs and early docs.
4. **Codex:** no verified contribution, so none is claimed.

Docs are written in the Wren plain-words style. Wren is a writing persona that
Claude runs. It is not an author or account.
