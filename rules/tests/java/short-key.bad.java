import java.security.KeyPairGenerator;
import java.security.Provider;

class KeyMaker {
    void make(Provider p) throws Exception {
        KeyPairGenerator g = KeyPairGenerator.getInstance("RSA");
        g.initialize(1024);
        KeyPairGenerator h = KeyPairGenerator.getInstance("RSA", p);
        h.initialize(1024);
        KeyPairGenerator.getInstance("DSA").initialize(1024);
    }
}
