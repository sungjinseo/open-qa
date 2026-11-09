import java.security.MessageDigest

fun digest(d: ByteArray): ByteArray = MessageDigest.getInstance("SHA-512").digest(d)
