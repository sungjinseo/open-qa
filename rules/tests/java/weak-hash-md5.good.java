import java.security.MessageDigest;
class H {
    byte[] h(byte[] d) throws Exception {
        return MessageDigest.getInstance("SHA-256").digest(d);
    }
}
