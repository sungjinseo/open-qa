import java.security.MessageDigest;

class Hash {
    MessageDigest make() throws Exception {
        return MessageDigest.getInstance("SHA-512");
    }
}
