import javax.crypto.Cipher

fun make(): Cipher = Cipher.getInstance("AES/ECB/PKCS5Padding")
