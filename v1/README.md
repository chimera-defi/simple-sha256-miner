# Version 1 — C++ proof of concept

`miner.cpp` searches SHA-256 digests of `gensys:<decimal nonce>` for a Base64 prefix of `gensys` using OpenSSL and six worker threads. On the measured host, throughput was approximately **2.66 million hashes/second**. It has no durable completed-range checkpoints; the logged attempt count is not a reliable resume position. Retained for historical comparison only.

The source requires a C++ compiler and an OpenSSL-compatible `libcrypto` library. See source for build assumptions. Do not run v1 and v2 against the same nonce range simultaneously.
