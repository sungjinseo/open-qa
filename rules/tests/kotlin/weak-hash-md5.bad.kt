import java.security.MessageDigest

fun digest(d: ByteArray): ByteArray = MessageDigest.getInstance("MD5").digest(d)
