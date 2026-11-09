import java.security.cert.*;
import javax.net.ssl.X509TrustManager;

class TrustAll implements X509TrustManager {
    public void checkClientTrusted(X509Certificate[] chain, String type) {}
    public void checkServerTrusted(X509Certificate[] chain, String type) {}
    public X509Certificate[] getAcceptedIssuers() { return new X509Certificate[0]; }
}
