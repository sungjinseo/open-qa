import socket

def internal(ip):
    if socket.gethostbyaddr(ip)[0].endswith(".corp.local"):
        return True
    return False
