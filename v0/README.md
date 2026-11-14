# v0

User-supplied v0 candidate (operator, in chat, 2026-11-14). Historical identity not independently verified.

The repository operator confirms the right to redistribute this file. Its
original authorship and source have not been independently verified.

- File: `miner.py`, SHA-256 `d32648c5c72fbe8fbf65957b03661c4ef65094f10ba0293ee54c59b62333f854`.
- Single-thread Python. Starts at nonce 0, hashes `SHA256("gensys:<unpadded decimal nonce>")`, stops at the first digest whose Base64 starts with `gensys`, and prints it.
- Run: `python3 miner.py` (no options, no resume).
