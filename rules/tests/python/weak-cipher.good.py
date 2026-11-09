from Crypto.Cipher import AES

def encrypt(key, data):
    cipher = AES.new(key, AES.MODE_GCM)
    return cipher.nonce, cipher.encrypt(data)
