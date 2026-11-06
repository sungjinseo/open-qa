import javax.crypto.Cipher;
class C {
    Cipher make() throws Exception {
        return Cipher.getInstance("DES");
    }
}
