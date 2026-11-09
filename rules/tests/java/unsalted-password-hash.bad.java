import java.security.MessageDigest;

class PwStore {
    byte[] hash(String password) throws Exception {
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        return md.digest(password.getBytes());
    }
}
