import java.security.KeyPairGenerator;

class KeyMaker {
    void make(Pool pool) throws Exception {
        KeyPairGenerator g = KeyPairGenerator.getInstance("RSA");
        g.initialize(3072);
        KeyPairGenerator ec = KeyPairGenerator.getInstance("EC");
        ec.initialize(256);
        pool.initialize(10);
    }
    interface Pool { void initialize(int n); }
}
