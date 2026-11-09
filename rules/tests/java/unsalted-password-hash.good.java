import org.springframework.security.crypto.bcrypt.BCryptPasswordEncoder;

class PwStore {
    String hash(String password) {
        return new BCryptPasswordEncoder().encode(password);
    }
}
