from Crypto.Cipher import AES

def encrypt(key, data):
    return AES.new(key, AES.MODE_ECB).encrypt(data)
