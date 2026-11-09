import javax.crypto.spec.SecretKeySpec;

class Keys {
    SecretKeySpec key() {
        return new SecretKeySpec("0123456789abcdef".getBytes(), "AES");
    }
}
