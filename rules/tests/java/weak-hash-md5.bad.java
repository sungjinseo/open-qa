import java.security.MessageDigest;
class H {
    byte[] h(byte[] d) throws Exception {
        return MessageDigest.getInstance("MD5").digest(d);
    }
}
