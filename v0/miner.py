import hashlib
import base64

nonce = 0

while True:
    message = f"gensys:{nonce}".encode()

    digest = hashlib.sha256(message).digest()
    encoded = base64.b64encode(digest).decode()

    if encoded.startswith("gensys"):
        print("Input:", message.decode())
        print("SHA256 Base64:", encoded)
        print("SHA256 Hex:", digest.hex())
        break

    nonce += 1
