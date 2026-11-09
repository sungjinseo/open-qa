import javax.crypto.Cipher;

class Enc {
    Cipher make() throws Exception {
        return Cipher.getInstance("AES/ECB/PKCS5Padding");
    }
}
