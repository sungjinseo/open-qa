import java.security.MessageDigest

fun digest(d: ByteArray): ByteArray = MessageDigest.getInstance("SHA-1").digest(d)
