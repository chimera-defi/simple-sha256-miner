# v1: C++ proof of concept

The first version. Kept for comparison. Use [v2](../v2/README.md) for real runs.

The repository operator confirms the right to redistribute this file. Its
original authorship has not been independently verified.

1. **What it does.** `miner.cpp` hashes `gensys:<decimal nonce>` with OpenSSL
   SHA-256 on six worker threads and looks for a Base64 prefix of `gensys`.
2. **Speed.** About 2.66 M hashes/s on one CPU, from its run logs (reported by
   ChatGPT, not re-measured by Claude).
3. **Weak spot.** It has no durable record of finished ranges. Its progress
   counter says how many chunks finished, not which ones, so it cannot prove
   where to resume.
4. **Build.** Needs a C++ compiler and an OpenSSL `libcrypto`. See the source
   for flags.
5. **Do not** run v1 and v2 on the same nonce range at once.

Historical behavior: Base64 prefix matching is **case-sensitive**. The original challenge permits case-insensitive matching; use v2 with `--ignore-case` for that mode.
