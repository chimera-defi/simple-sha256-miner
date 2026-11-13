#!/usr/bin/env python3
"""Independent check: python3 -I validate.py <state-dir>/FOUND.txt  (exact, case-sensitive)."""
import sys, hashlib, base64
kv = dict(l.rstrip("\n").split("=", 1) for l in open(sys.argv[1]) if "=" in l)
d = hashlib.sha256(kv["input"].encode()).digest()
b = base64.b64encode(d).decode()
assert d.hex() == kv["sha256_hex"], "hex mismatch"
assert b == kv["base64"], "b64 mismatch"
assert b.startswith(kv["prefix"]), "PREFIX MISMATCH"
assert kv["input"] == "gensys:" + kv["nonce"]
print("VALID", kv["input"], b)
