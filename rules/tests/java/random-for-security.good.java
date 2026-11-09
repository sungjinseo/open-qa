import java.security.SecureRandom;
class T {
    int token() {
        return new SecureRandom().nextInt();
    }
}
