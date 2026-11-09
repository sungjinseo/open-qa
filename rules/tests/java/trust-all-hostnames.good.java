import javax.net.ssl.HttpsURLConnection;

class Tls {
    void init(java.net.URL u) throws Exception {
        HttpsURLConnection c = (HttpsURLConnection) u.openConnection();
        c.connect();
    }
}
