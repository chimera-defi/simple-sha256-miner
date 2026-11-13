# Simple SHA-256 Miner

Small CPU search for a vanity hash. It tries nonces until the **Base64 form of
`SHA-256(<phrase>:<nonce>)` starts with a chosen prefix**. It finds a real
matching hash; it never edits the digest. This is not Bitcoin mining.

**Use v2.** It is far faster than v1, and a stop or crash does not lose finished work.

| | Language | Speed (6 threads) | Resume |
| --- | --- | ---: | --- |
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
`--encoding base64|hex`, `--backend lib|avx2` (default `lib`; `avx2` is opt-in,
x86-64 and sha256 only). Base64 prefixes are case-sensitive; hex is lowercase.
Each extra prefix character makes the expected work 64× (base64) or 16× (hex)
larger. No promise of when a match appears.

## Not built yet

- Matching anywhere but the start of the digest.
- Multi-lane BLAKE3 (it hashes one nonce at a time).

## Who did what

1. **Chimera (operator):** set the goal, approved the work, owns the repo.
2. **Claude (Wren):** engineering, tests, and documentation (v2 search, journal and checkpoint, validator).
3. **ChatGPT:** coordination, benchmark runs and early docs.
4. **Codex:** no verified contribution, so none is claimed.
