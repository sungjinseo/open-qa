import javax.net.ssl.*;
class T {
    void init() {
        HttpsURLConnection.setDefaultHostnameVerifier((h, s) -> true);
    }
}
