import java.security.SecureRandom;

class TokenMaker {
    String token() {
        return Long.toHexString(new SecureRandom().nextLong());
    }
}
