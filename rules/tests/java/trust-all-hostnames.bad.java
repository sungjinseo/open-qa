import javax.net.ssl.HttpsURLConnection;

class Tls {
    void init() {
        HttpsURLConnection.setDefaultHostnameVerifier((host, session) -> true);
    }
}
