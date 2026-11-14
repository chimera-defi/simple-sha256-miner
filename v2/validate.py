#!/usr/bin/env python3
"""Independent sha256 check: python3 -I validate.py < proof.json  (blake3 is not in the stdlib)."""
import sys, json, hashlib, base64
p = json.load(sys.stdin)
assert p["algorithm"] == "sha256", "only sha256 is checked here"
d = hashlib.sha256(f'{p["input"]}:{p["nonce"]}'.encode()).digest()
enc = base64.b64encode(d).decode() if p["encoding"] == "base64" else d.hex()
assert enc == p["digest"], "digest mismatch"
mode = p.get("ignore_case", False)
assert isinstance(mode, bool), "ignore_case must be boolean"
assert not mode or p["encoding"] == "base64", "ignore_case requires base64"
prefix = p["prefix"]
assert isinstance(prefix, str), "prefix must be a string"
if mode:
    assert 1 <= len(prefix) <= 10 and all(c in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/" for c in prefix), "invalid base64 prefix"
    assert enc[:len(prefix)].lower() == prefix.lower(), "PREFIX MISMATCH"
else:
    assert enc.startswith(prefix), "PREFIX MISMATCH"
print("VALID", p["input"], p["nonce"], enc)
