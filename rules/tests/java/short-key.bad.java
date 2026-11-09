import java.security.KeyPairGenerator;

class KeyMaker {
    void make() throws Exception {
        KeyPairGenerator g = KeyPairGenerator.getInstance("RSA");
        g.initialize(1024);
    }
}
