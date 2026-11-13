#!/usr/bin/env python3
"""Independent sha256 check: python3 -I validate.py < proof.json  (blake3 is not in the stdlib)."""
import sys, json, hashlib, base64
p = json.load(sys.stdin)
assert p["algorithm"] == "sha256", "only sha256 is checked here"
d = hashlib.sha256(f'{p["input"]}:{p["nonce"]}'.encode()).digest()
enc = base64.b64encode(d).decode() if p["encoding"] == "base64" else d.hex()
assert enc == p["digest"], "digest mismatch"
assert enc.startswith(p["prefix"]), "PREFIX MISMATCH"
print("VALID", p["input"], p["nonce"], enc)
