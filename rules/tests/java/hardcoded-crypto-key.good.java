import javax.crypto.spec.SecretKeySpec;
import java.util.Base64;

class Keys {
    SecretKeySpec key() {
        return new SecretKeySpec(Base64.getDecoder().decode(System.getenv("APP_KEY")), "AES");
    }
}
