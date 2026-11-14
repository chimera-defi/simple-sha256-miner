#!/usr/bin/env python3
"""Independent Python proof-validator regression checks; no mining."""
import base64
import hashlib
import json
import subprocess
import sys
from pathlib import Path

validator = Path(__file__).resolve().parents[1] / "validate.py"
raw = hashlib.sha256(b"gensys:0").digest()
encoded = base64.b64encode(raw).decode("ascii")

def check(proof, accepted):
    result = subprocess.run([sys.executable, "-I", str(validator)], input=json.dumps(proof), text=True, capture_output=True, check=False)
    assert (result.returncode == 0) == accepted, (proof, result.stdout, result.stderr)

base = {"input": "gensys", "nonce": 0, "algorithm": "sha256", "encoding": "base64", "digest": encoded}
for length in range(1, 11):
    original = encoded[:length]
    flipped = original.swapcase()
    proof = {**base, "prefix": flipped, "ignore_case": True}
    check(proof, True)
    check({**proof, "ignore_case": False}, flipped == original)
    check({k: v for k, v in proof.items() if k != "ignore_case"}, flipped == original)
    check({**proof, "digest": "invalid"}, False)
    check({**proof, "ignore_case": "true"}, False)
    check({**proof, "encoding": "hex"}, False)
check({**base, "prefix": encoded[:4]}, True)
check({**base, "prefix": "!", "ignore_case": True}, False)
print("Python proof-validator regression: PASS")
