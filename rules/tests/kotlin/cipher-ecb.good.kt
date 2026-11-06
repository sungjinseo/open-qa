import javax.crypto.Cipher

fun make(): Cipher = Cipher.getInstance("AES/GCM/NoPadding")
