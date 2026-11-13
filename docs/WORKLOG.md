# Worklog

Single canonical follow-up tracker. Any agent: update the row, do not start another list.

| Item | Status | Priority | Owner | Evidence | Next action | Refs |
| --- | --- | --- | --- | --- | --- | --- |
| Upstream `hash_short_heads` patch to tape-sha256 | OPEN, needs operator approval | normal | operator decides | patch in `v2/vendor/tape-sha256/PATCH.diff`; no upstream PR exists | ask operator; then open PR | 4965fa6; task-idem-9208dd0d7edc53fe3aeb76ad turn-12 |
| v0 provenance | identity unverified, operator-supplied | low | operator | `v0/miner.py` sha256 `d32648c5c72fbe8fbf65957b03661c4ef65094f10ba0293ee54c59b62333f854`, committed unchanged | none unless operator can name its source | bus 14262 |
| Nonce `u64::MAX` itself | OPEN, accepted limit | low | Claude | v2 covers `[0, 2^64-1)`; a half-open interval cannot name `u64::MAX`, so that single nonce is never tried | none unless the full domain is required | 4965fa6; checkpoint bus 14264 |
| Multi-lane BLAKE3 | OPEN | low | unassigned | `--algorithm blake3` hashes one nonce at a time | build only if BLAKE3 speed matters | v2/src/lib.rs |
| Public ELI5 site | PREPARATION approved, publication NOT approved | normal | operator decides | `docs/performance-eli5.html` (static, no scripts or external assets). Plan: a separate new public repo holding only that page (no code, no history from this repo), leak scan of the page, then GitHub Pages. This repo stays private | operator approval to create the public repo and enable Pages; link it from this README only after the URL is approved | bus 14266, 14269 |
| Backend choice | DONE: offer both, `lib` default | n/a | Claude | `--backend lib\|avx2` in v2, tests in `v2/tests/core.rs`, speeds in `v2/README.md` | none | ruling bus 14248; restored in the commit that adds this file |

## Input formats

v0, v1 and v2 all hash `<input>:<unpadded decimal nonce>`. Checked: `v0/miner.py` uses `f"gensys:{nonce}"`; `v1/miner.cpp` line 35 uses `snprintf(msg, ..., "gensys:%llu", nonce)`; v2 uses `<input>:<nonce>` with `--input` defaulting to `gensys`. The default input is `gensys`. The Base64 prefix is case-sensitive. v1 and v2 start at nonce 0 (v2 can start elsewhere with `--start-nonce`).
